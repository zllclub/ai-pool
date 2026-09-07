import React from 'react';
import ReactDOM from 'react-dom/client';
import { AccountsPage } from './pages/AccountsPage';
import { AccountWidget } from './components/AccountWidget';
import ConfigProvider from '@arco-design/web-react/es/ConfigProvider';
import zhCN from '@arco-design/web-react/es/locale/zh-CN';
// Load only the component styles used by this screen (including their dependencies).
import '@arco-design/web-react/es/Button/style/css.js';
import '@arco-design/web-react/es/Card/style/css.js';
import '@arco-design/web-react/es/Avatar/style/css.js';
import '@arco-design/web-react/es/Dropdown/style/css.js';
import '@arco-design/web-react/es/Menu/style/css.js';
import '@arco-design/web-react/es/Tag/style/css.js';
import '@arco-design/web-react/es/Tooltip/style/css.js';
import '@arco-design/web-react/es/Progress/style/css.js';
import '@arco-design/web-react/es/InputNumber/style/css.js';
import '@arco-design/web-react/es/Popover/style/css.js';
import '@arco-design/web-react/es/Modal/style/css.js';
import '@arco-design/web-react/es/Alert/style/css.js';
import '@arco-design/web-react/es/Spin/style/css.js';
import '@arco-design/web-react/es/Empty/style/css.js';
import './styles.css';
const isWidget = new URLSearchParams(window.location.search).has('widget');
if (isWidget) {
  document.documentElement.classList.add('widget-document');
  document.body.classList.add('widget-body');
}
ReactDOM.createRoot(document.getElementById('root')!).render(<React.StrictMode><ConfigProvider locale={zhCN} size="small">{isWidget ? <AccountWidget /> : <AccountsPage />}</ConfigProvider></React.StrictMode>);
