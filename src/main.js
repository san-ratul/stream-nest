import { createApp } from "vue";
import App from "./App.vue";
import { router } from "./router";
import { startTorrentStore } from "./stores/torrents";
import "./style.css";

startTorrentStore().finally(() => {
  createApp(App).use(router).mount("#app");
});
