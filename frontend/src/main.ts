import { createApp } from 'vue';
import { pinia } from '@/store';

import '@/styles/settings.scss';
import App from '@/App.vue';
import { i18n } from '@/plugins/i18n';
import { vuetify } from '@/plugins/vuetify';

const app = createApp(App);

app.use(i18n);
app.use(vuetify);
app.use(pinia);
app.mount('#app');
