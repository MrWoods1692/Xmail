// Tauri doesn't have a Node.js server to do proper SSR
// so we will use adapter-static to prerender the app (SSG)
// See: https://v2.tauri.app/start/frontend/sveltekit/ for more info
import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter(),
  },
  compilerOptions: {
    warningFilter: (warning) => {
      // 忽略模态对话框的无障碍访问警告
      if (warning.code === 'a11y-click-events-have-key-events' ||
          warning.code === 'a11y-no-static-element-interactions') {
        return false;
      }
      return true;
    }
  }
};

export default config;
