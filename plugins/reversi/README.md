# Reversi / 黑白棋

A complete first-party page plugin for EchoBird. Source, manifest and runtime files are in this directory. The built-in project demonstration in EchoBird remains available independently.

## Install in a development build

This requires EchoBird's plugin runtime from the development branch; existing released clients without the plugin page cannot install this package.

- In **Developer preview → Plugins**, install `echobird.reversi` from the official Hub, or choose **Add from local directory** and select this directory.
- Repository import accepts `https://github.com/edison7009/echobird-plugin#plugins/reversi`. The China mirror is `https://gitcode.com/edison7009/echobird-plugin#plugins/reversi`. Repository import requires Git; Hub package installation does not.
- Select the new **黑白棋 · Reversi** entry on the application desktop, choose a model, and launch it. A move requests a short text response from that selected model. Model providers may charge for these requests.
- Disable or uninstall the plugin from the plugin page to remove its desktop entry. The built-in **Projects** demonstration is retained.

The package requests only `model.request`. It uses `window.echobird.generateText(prompt)` and receives text, never a model API key, account configuration or arbitrary filesystem/process access. Closing or disabling the plugin cancels pending host requests. Model failures are shown in the game. Installation and page loading do not send model requests.

## 中文

这是可以安装和运行的完整示例。复制本目录即可开发自己的页面插件，入口为 `ui/index.html`，清单为 `package.json`。本地安装复制快照，修改源码后需要重新添加。插件不运行 `npm install`、构建脚本或生命周期脚本；请在提交前自行构建需要的静态资源。

在支持插件运行时的开发版中安装后，从应用桌面选择插件、选择模型并启动。点击棋盘才请求模型；失败会显示在棋盘旁。停用/卸载不影响“项目”中的内置演示。首次运行接口仍为 `0.1-draft`，Windows 原生验收和 macOS/Linux 的实际验证状态见客户端交付记录；清单声明不等于所有系统已实机测试。

The plugin uses the [MIT license](LICENSE).
