import type { RouteRecordRaw } from 'vue-router'
import App from '@/pages/app.vue'
import Preference from '@/pages/preference.vue'
import PreferenceRoute from '@/prefComponents/preferenceRoute.vue'
import { preferenceCategories } from '@/prefComponents/categories'

const parseSettingsPage = (type: string | null | undefined): string => {
  const category = type?.split('/')[1]
  return `/preference/${preferenceCategories.find(item => item.id === category)?.id ?? 'general'}`
}

const routes = (type: string | null | undefined): RouteRecordRaw[] => [
  {
    path: '/',
    redirect: type === 'editor' ? '/editor' : parseSettingsPage(type)
  },
  { path: '/editor', component: App },
  {
    path: '/preference',
    component: Preference,
    children: [
      { path: '', redirect: '/preference/general' },
      ...preferenceCategories.map(category => ({
        path: category.id,
        name: category.id,
        component: PreferenceRoute,
        props: { category: category.id }
      }))
    ]
  }
]

export default routes
