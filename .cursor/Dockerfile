FROM ubuntu:24.04

ENV DEBIAN_FRONTEND=noninteractive
ENV USE_ELECTRON_CLANG=0

# 1. Core build tools, MarkText native deps (ced, keytar, native-keymap),
#    Electron headless runtime (Xvfb + Chromium shared libs), Linux GPUI deps
#    (X11, Wayland, Vulkan, Mesa software rasterizer), fast LLVM linker (lld),
#    Liberation fonts (GPUI text), screenshot tools (scrot), and GNU time.
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    curl \
    git \
    gnupg \
    sudo \
    unzip \
    python3 \
    build-essential \
    pkg-config \
    cmake \
    clang \
    lld \
    time \
    xvfb \
    scrot \
    fonts-liberation \
    libx11-dev \
    libxkbfile-dev \
    libsecret-1-dev \
    libfontconfig-dev \
    libnss3 \
    libatk1.0-0 \
    libatk-bridge2.0-0 \
    libcups2 \
    libdrm2 \
    libgtk-3-0 \
    libgbm1 \
    libasound2t64 \
    libxshmfence1 \
    libxcomposite1 \
    libxdamage1 \
    libxfixes3 \
    libxrandr2 \
    libpango-1.0-0 \
    libcairo2 \
    libxkbcommon-dev \
    libxkbcommon-x11-dev \
    libwayland-dev \
    libvulkan-dev \
    mesa-vulkan-drivers \
    libgl1-mesa-dri \
    libxcb1-dev \
    libxcb-render0-dev \
    libxcb-shape0-dev \
    libxcb-xfixes0-dev \
    && rm -rf /var/lib/apt/lists/* \
    && ARCH="$(dpkg --print-architecture)" \
    && if [ ! -e /usr/share/vulkan/icd.d/lvp_icd.json ]; then \
         for candidate in \
           "/usr/share/vulkan/icd.d/lvp_icd.${ARCH}.json" \
           "/usr/share/vulkan/icd.d/lvp_icd.x86_64.json" \
           "/usr/share/vulkan/icd.d/lvp_icd.aarch64.json"; do \
           if [ -f "$candidate" ]; then ln -sf "$candidate" /usr/share/vulkan/icd.d/lvp_icd.json; break; fi; \
         done; \
       fi

ENV VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json

# 2. Node.js 22.x (matches MarkText CI), pnpm 10.33.4, and global node-gyp
RUN curl -fsSL https://deb.nodesource.com/setup_22.x | bash - \
    && apt-get install -y --no-install-recommends nodejs \
    && rm -rf /var/lib/apt/lists/* \
    && corepack enable \
    && corepack prepare pnpm@10.33.4 --activate \
    && npm install -g node-gyp

# 3. Non-root ubuntu user (matches Cursor Cloud Agent convention)
RUN id -u ubuntu >/dev/null 2>&1 || useradd -m -s /bin/bash ubuntu \
    && usermod -aG sudo ubuntu \
    && echo "ubuntu ALL=(ALL) NOPASSWD:ALL" > /etc/sudoers.d/ubuntu \
    && chmod 0440 /etc/sudoers.d/ubuntu

# 4. Prebuilt wasm-bindgen-cli 0.2.130 (matches gpui-ce 0.3.0 / wasm-bindgen 0.2.130)
RUN ARCH="$(dpkg --print-architecture)" \
    && if [ "$ARCH" = "arm64" ]; then TARGET="aarch64-unknown-linux-musl"; else TARGET="x86_64-unknown-linux-musl"; fi \
    && curl -fsSL "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/0.2.130/wasm-bindgen-0.2.130-${TARGET}.tar.gz" \
      | tar -xz --strip-components=1 -C /usr/local/bin "wasm-bindgen-0.2.130-${TARGET}/wasm-bindgen" "wasm-bindgen-0.2.130-${TARGET}/wasm-bindgen-test-runner" \
    && chmod +x /usr/local/bin/wasm-bindgen /usr/local/bin/wasm-bindgen-test-runner

# 5. Rust stable toolchain (>=1.95 for gpui-ce 0.3.0) + wasm32-unknown-unknown target
#    + Linux lld linker config and bounded parallel jobs (keeps `cargo test` peak RSS well under container RAM)
USER ubuntu
ENV HOME=/home/ubuntu
ENV CARGO_HOME=/home/ubuntu/.cargo
ENV RUSTUP_HOME=/home/ubuntu/.rustup
ENV PATH="/home/ubuntu/.cargo/bin:${PATH}"

RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --default-toolchain stable --profile minimal \
    && rustup component add rustfmt clippy \
    && rustup target add wasm32-unknown-unknown \
    && mkdir -p /home/ubuntu/.cargo \
    && printf '%s\n' \
      '[build]' \
      'jobs = 3' \
      '' \
      '[target.x86_64-unknown-linux-gnu]' \
      'linker = "clang"' \
      'rustflags = ["-C", "link-arg=-fuse-ld=lld"]' \
      '' \
      '[target.aarch64-unknown-linux-gnu]' \
      'linker = "clang"' \
      'rustflags = ["-C", "link-arg=-fuse-ld=lld"]' \
      > /home/ubuntu/.cargo/config.toml \
    && rustc --version && cargo --version && wasm-bindgen --version && ld.lld --version
