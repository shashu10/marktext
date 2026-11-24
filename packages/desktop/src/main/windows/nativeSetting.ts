import { spawn, type ChildProcessWithoutNullStreams } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { createInterface } from 'node:readline'
import { dialog, shell } from 'electron'
import log from 'electron-log'
import { isOsx } from '../config'
import type Accessor from '../app/accessor'
import BaseWindow, { WindowLifecycle, WindowType } from './base'
import { isExecutableFile } from 'common/filesystem'
import { resolveCommand } from '../utils/resolveCommand'

type JsonObject = Record<string, unknown>

interface GuestMessage extends JsonObject {
  type: string
}

/** Prefer an explicit override, then release binary next to resources, then cargo target. */
export const resolvePrefAppBinary = (
  env: NodeJS.ProcessEnv = process.env,
  // electron-vite emits main into packages/desktop/out/main → repo root is ../../../../
  repoHint = path.resolve(__dirname, '../../../..')
): string | null => {
  const override = env.MARKTEXT_PREF_APP?.trim()
  if (override && fs.existsSync(override)) return override
  const candidates = [
    path.join(process.resourcesPath || '', 'mt-pref-app'),
    path.join(repoHint, 'rust/target/release/mt-pref-app'),
    path.join(repoHint, 'rust/target/debug/mt-pref-app')
  ]
  for (const candidate of candidates) {
    if (candidate && fs.existsSync(candidate)) return candidate
  }
  return null
}

const isRecord = (value: unknown): value is JsonObject =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

let nextNativeId = 0x4e415400 // 'NAT\0'

/**
 * Stage 2 preferences window: spawns `mt-pref-app` and speaks HostToGuest /
 * GuestToHost as NDJSON on the child stdio (NativeIpcBridge).
 */
class NativeSettingWindow extends BaseWindow {
  private child: ChildProcessWithoutNullStreams | null = null
  private category: string = 'general'
  private closing = false

  constructor(accessor: Accessor) {
    super(accessor)
    this.type = WindowType.SETTINGS
  }

  createWindow(category: string | null = null): null {
    const binary = resolvePrefAppBinary()
    if (!binary) {
      throw new Error(
        'mt-pref-app binary not found. Build with `pnpm migration:build-native-pref` or set MARKTEXT_PREF_APP.'
      )
    }

    this.category = category && category.length > 0 ? category : 'general'
    this.id = nextNativeId++
    this.browserWindow = null
    this.lifecycle = WindowLifecycle.LOADING

    const child = spawn(binary, [this.category], {
      stdio: ['pipe', 'pipe', 'pipe'],
      // Keep the child in Electron's process group so a hard exit reaps it.
      detached: false,
      env: {
        ...process.env,
        // Headless Cloud VMs need lavapipe; hardware GPUs ignore this ICD list.
        VK_ICD_FILENAMES:
          process.env.VK_ICD_FILENAMES ||
          '/usr/share/vulkan/icd.d/lvp_icd.json'
      }
    })
    this.child = child

    child.stderr.on('data', (chunk: Buffer) => {
      log.info(`[mt-pref-app] ${chunk.toString('utf8').trimEnd()}`)
    })
    child.on('exit', (code, signal) => {
      this.child = null
      if (!this.closing) {
        log.info(`mt-pref-app exited code=${code} signal=${signal}`)
        this.emit('window-closed')
      }
      this.lifecycle = WindowLifecycle.QUITTED
      this.id = null
    })

    const lines = createInterface({ input: child.stdout })
    lines.on('line', (line) => {
      void this.onGuestLine(line)
    })

    this.lifecycle = WindowLifecycle.READY
    this.emit('window-ready')
    return null
  }

  changeCategory(category: string | null | undefined): void {
    if (!category) return
    this.category = category
    this.send({ type: 'SET_PANE', pane: category })
    this.send({ type: 'ACTIVE_CATEGORY', category })
  }

  override bringToFront(): void {
    this.emit('window-focus')
  }

  override reload(): void {
    // Native prefs reload by re-pushing the snapshot; no BrowserWindow to reload.
    void this.pushBootstrap()
  }

  override destroy(): void {
    this.closing = true
    if (this.child && !this.child.killed) {
      this.child.kill()
    }
    this.child = null
    this.emit('window-closed')
    this.lifecycle = WindowLifecycle.QUITTED
  }

  send(message: JsonObject): void {
    if (!this.child?.stdin.writable) return
    this.child.stdin.write(`${JSON.stringify(message)}\n`)
  }

  private async onGuestLine(line: string): Promise<void> {
    const trimmed = line.trim()
    if (!trimmed) return
    let message: GuestMessage
    try {
      message = JSON.parse(trimmed) as GuestMessage
    } catch {
      log.warn('mt-pref-app: drop malformed guest line')
      return
    }
    if (typeof message.type !== 'string') return

    if (message.type === 'READY') {
      await this.pushBootstrap()
      return
    }
    if (message.type === 'SET_PREFERENCE' && typeof message.key === 'string') {
      this._accessor.preferences.setItems({ [message.key]: message.value })
      if (message.key === 'spellcheckerLanguage' && typeof message.value === 'string') {
        // Editor windows pick this up via the preference broadcast.
      }
      return
    }
    if (
      message.type === 'PERSIST_PREFERENCE' &&
      typeof message.id === 'number' &&
      typeof message.key === 'string'
    ) {
      const patch =
        message.key === '' && isRecord(message.value)
          ? message.value
          : { [message.key]: message.value }
      const result = this._accessor.preferences.update(patch)
      this.send({
        type: 'PREFERENCE_RESULT',
        id: message.id,
        status: result.status === 'persisted' ? 'persisted' : 'failed'
      })
      return
    }
    if (message.type === 'CHOOSE_DIRECTORY' && typeof message.id === 'number') {
      const result = await dialog.showOpenDialog({ properties: ['openDirectory'] })
      this.send({
        type: 'DIRECTORY_CHOSEN',
        id: message.id,
        path: result.canceled ? null : (result.filePaths[0] ?? null)
      })
      return
    }
    if (
      message.type === 'HOST_ACTION' &&
      typeof message.id === 'number' &&
      typeof message.action === 'string'
    ) {
      await this.runHostAction(message.id, message.action, message.payload)
      return
    }
    if (message.type === 'REQUEST_KEYBINDINGS') {
      this.sendKeybindings()
      return
    }
    if (
      message.type === 'SAVE_KEYBINDING' &&
      typeof message.id === 'number' &&
      typeof message.commandId === 'string' &&
      typeof message.accelerator === 'string'
    ) {
      await this.saveKeybinding(message.id, message.commandId, message.accelerator)
      return
    }
    if (message.type === 'RESET_ALL_KEYBINDINGS' && typeof message.id === 'number') {
      await this.resetAllKeybindings(message.id)
      return
    }
    if (message.type === 'SPELLCHECKER_REMOVE_WORD' && typeof message.id === 'number') {
      // Custom dictionary IPC lives on renderer spellchecker today; acknowledge failure.
      this.send({ type: 'SPELLCHECKER_REMOVE_RESULT', id: message.id, ok: false })
      return
    }
    if (message.type === 'KEYBINDING_DEBUG_DUMP') {
      // Handled by the existing keyboard debug IPC when a renderer asks; no-op here.
      return
    }
  }

  private async pushBootstrap(): Promise<void> {
    const preferences = {
      ...this._accessor.preferences.getAll(),
      ...(await this._accessor.dataCenter.getAll())
    }
    this.send({ type: 'HOST_HELLO' })
    this.send({ type: 'SET_PANE', pane: this.category })
    this.send({ type: 'ACTIVE_CATEGORY', category: this.category })
    this.send({ type: 'PREFERENCES', snapshot: preferences })
    this.sendKeybindings()
    this.send({
      type: 'SPELLCHECKER_INFO',
      isOsx: isOsx,
      dictionaries: isOsx ? [] : ['en-US'],
      words: []
    })
  }

  private sendKeybindings(): void {
    const { keybindings, env } = this._accessor
    const defaults = keybindings.getDefaultKeybindings()
    const user = keybindings.getUserKeybindings()
    const commands: Array<{
      id: string
      description: string
      accelerator: string
      defaultAccelerator: string
    }> = []
    for (const [id, defaultAccelerator] of defaults) {
      if (!isOsx && id.startsWith('mt.')) continue
      const userAccelerator = user.get(id)
      const accelerator = userAccelerator != null ? userAccelerator : defaultAccelerator
      commands.push({
        id,
        description: id,
        accelerator,
        defaultAccelerator
      })
    }
    this.send({
      type: 'KEYBINDINGS',
      commands,
      isMac: isOsx,
      showDebug: Boolean(env.debug),
      loading: false,
      loadError: false
    })
  }

  private async saveKeybinding(id: number, commandId: string, accelerator: string): Promise<void> {
    const { keybindings, menu, windowManager } = this._accessor
    const next = new Map(keybindings.getUserKeybindings())
    const defaults = keybindings.getDefaultKeybindings()
    if (accelerator === '' || accelerator === defaults.get(commandId)) {
      next.delete(commandId)
    } else {
      next.set(commandId, accelerator)
    }
    const editorWindows = windowManager
      .getWindowsByType(WindowType.EDITOR)
      .map(({ win }) => win.browserWindow)
      .filter((win): win is NonNullable<typeof win> => win != null)
    const saved = await keybindings.setUserKeybindings(next, editorWindows)
    menu.updateKeybindings()
    this.send({
      type: 'KEYBINDING_RESULT',
      id,
      status: saved ? 'persisted' : 'failed',
      commands: undefined
    })
    this.sendKeybindings()
  }

  private async resetAllKeybindings(id: number): Promise<void> {
    const { keybindings, menu, windowManager } = this._accessor
    const editorWindows = windowManager
      .getWindowsByType(WindowType.EDITOR)
      .map(({ win }) => win.browserWindow)
      .filter((win): win is NonNullable<typeof win> => win != null)
    const saved = await keybindings.setUserKeybindings(new Map(), editorWindows)
    menu.updateKeybindings()
    this.send({
      type: 'KEYBINDING_RESULT',
      id,
      status: saved ? 'persisted' : 'failed'
    })
    this.sendKeybindings()
  }

  private async runHostAction(id: number, action: string, payload: unknown): Promise<void> {
    const body = isRecord(payload) ? payload : {}
    let status = 'failed'
    let result: JsonObject = {}
    try {
      if (action === 'navigate-preference' && typeof body.category === 'string') {
        this.changeCategory(body.category)
        status = 'ok'
        result = {
          category: body.category,
          ...(typeof body.key === 'string' ? { key: body.key } : {})
        }
      } else if (action === 'set-user-data' && typeof body.key === 'string') {
        this._accessor.dataCenter.setItems({ [body.key]: body.value })
        status = 'ok'
        result = { [body.key]: body.value }
      } else if (action === 'ask-image-folder') {
        let imagePath = typeof body.path === 'string' ? body.path : undefined
        if (!imagePath) {
          const picked = await dialog.showOpenDialog({
            properties: ['openDirectory', 'createDirectory']
          })
          imagePath = picked.canceled ? undefined : picked.filePaths[0]
        }
        if (imagePath) {
          this._accessor.dataCenter.setItems({ imageFolderPath: imagePath })
          status = 'ok'
          result = { imageFolderPath: imagePath }
        } else {
          status = 'ok'
          result = {}
        }
      } else if (action === 'open-path' && typeof body.path === 'string') {
        await shell.openPath(body.path)
        status = 'ok'
      } else if (action === 'open-external' && typeof body.url === 'string') {
        await shell.openExternal(body.url)
        status = 'ok'
      } else if (action === 'probe-pandoc') {
        const command = await resolveCommand('pandoc')
        status = 'ok'
        result = { command: command ?? '' }
      } else if (action === 'is-executable' && typeof body.path === 'string') {
        status = 'ok'
        result = { path: body.path, executable: isExecutableFile(body.path) }
      } else if (action === 'probe-picgo') {
        status = 'ok'
        result = { exists: false, detectedAt: new Date().toISOString(), debug: 'native-host' }
      }
    } catch (error) {
      status = 'failed'
      result = {
        error: error instanceof Error ? error.message : String(error)
      }
    }
    this.send({ type: 'HOST_ACTION_RESULT', id, status, payload: result })
  }
}

export default NativeSettingWindow
