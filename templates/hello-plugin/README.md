# Hello Plugin

A self-contained static page and a draft manifest. It makes no host API calls and requests no permissions. It is not an installable integration with the current EchoBird client.

1. Copy this directory into your own repository.
2. Replace `publisher`, `name`, `repository` and display metadata in `package.json`.
3. Edit `ui/index.html`. Open it in a browser to preview the page.
4. From this Hub checkout, validate the copied manifest:

   ```sh
   node scripts/validate.mjs --manifest /absolute/path/to/your-plugin/package.json
   ```

The manifest API is `0.1-draft`. Host loading, permissions, lifecycle and extension interfaces remain under design. Do not claim compatibility with a released EchoBird version.

## 中文

这是静态页面与草案清单模板，无宿主接口调用或权限请求。复制目录后修改发布者、名称、仓库地址和页面；可在浏览器单独预览。当前 EchoBird 客户端尚未实现插件加载，模板不代表已完成客户端接入。

For a theme or background extension, use [Ocean Theme](../../plugins/ocean-theme) or [Text Tools](../../plugins/text-tools) as a starting point and follow the [product SDK contract](../../docs/plugin-spec.md#product-extensions). Keep permissions limited to the APIs your plugin uses.
