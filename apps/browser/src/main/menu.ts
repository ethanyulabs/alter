// 应用菜单：快捷键都挂在菜单项的 accelerator 上，焦点在页面视图里时也生效（Renderer 收不到那时的按键）
import { Menu, type MenuItemConstructorOptions } from "electron";

import type { Actions } from "./actions";

/** 菜单项 id 与快捷键（N1 R12 的表；N4 起 tab.new 打开命令栏）。 */
export function buildMenu(actions: Actions): Menu {
  const spaceItems: MenuItemConstructorOptions[] = Array.from({ length: 9 }, (_, i) => ({
    id: `space.${i + 1}`,
    label: `切到第 ${i + 1} 个 Space`,
    accelerator: `CmdOrCtrl+${i + 1}`,
    click: () => actions.activateSpaceAt(i + 1),
  }));
  const template: MenuItemConstructorOptions[] = [
    // macOS 的应用菜单与编辑菜单保留系统默认项，否则退出、复制粘贴等快捷键会失效
    { role: "appMenu" },
    {
      label: "标签",
      submenu: [
        { id: "tab.new", label: "新标签…", accelerator: "CmdOrCtrl+T", click: () => actions.openCommandBar() },
        { id: "tab.close", label: "关闭标签", accelerator: "CmdOrCtrl+W", click: () => actions.closeActiveTab() },
        {
          id: "tab.togglePin",
          label: "固定 / 取消固定",
          accelerator: "CmdOrCtrl+Shift+D",
          click: () => actions.togglePinActive(),
        },
        { type: "separator" },
        { id: "tab.next", label: "下一个标签", accelerator: "Ctrl+Tab", click: () => actions.cycleTab(1) },
        { id: "tab.prev", label: "上一个标签", accelerator: "Ctrl+Shift+Tab", click: () => actions.cycleTab(-1) },
      ],
    },
    { role: "editMenu" },
    {
      label: "导航",
      submenu: [
        { id: "address.focus", label: "地址栏", accelerator: "CmdOrCtrl+L", click: () => actions.focusAddress(false) },
        { id: "nav.reload", label: "刷新", accelerator: "CmdOrCtrl+R", click: () => actions.nav("reload") },
        { id: "nav.back", label: "后退", accelerator: "CmdOrCtrl+[", click: () => actions.nav("back") },
        { id: "nav.forward", label: "前进", accelerator: "CmdOrCtrl+]", click: () => actions.nav("forward") },
      ],
    },
    { label: "Space", submenu: spaceItems },
    { role: "windowMenu" },
  ];
  return Menu.buildFromTemplate(template);
}
