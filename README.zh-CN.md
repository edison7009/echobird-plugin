# EchoBird 插件 Hub

[English](README.md)

这里维护 EchoBird 插件规范、开发模板和已审核插件索引。

**当前状态：** 页面接口仍为实验性 `0.1-draft`，安装与页面运行时正在未发布的 EchoBird 开发版中接入。没有该运行时的已发布客户端无法安装插件。[黑白棋](plugins/reversi/README.md) 是首个完整插件，也可作为开发示例；“项目”中的内置演示独立保留。

## 中国区与国际区

| 区域           | 仓库                                                      |
| -------------- | --------------------------------------------------------- |
| 中国区分发镜像 | [GitCode](https://gitcode.com/edison7009/echobird-plugin) |
| 国际区主线     | [GitHub](https://github.com/edison7009/echobird-plugin)   |

两边保持相同的 `index.json`、插件标识和版本。每个上架版本需要国际区、中国区下载地址，共用一个 SHA-256 校验值。只同步 Git 代码不会复制发布附件、Issue 或 PR；中国区安装包需要单独分发并验证一致。

## 开发与提交

1. 复制 [Hello Plugin 模板](templates/hello-plugin/README.md)，在自己的 Git 仓库开发。
   也可以参考完整的 [黑白棋源码](plugins/reversi/README.md) 和 [宿主接口类型](sdk/echobird.d.ts)。
2. 按 [插件规范](docs/plugin-spec.md) 修改清单；源码可托管在 GitHub、CNB、GitCode 或其他公共 Git 平台。
3. 按 [索引规范](docs/registry-spec.md) 准备固定提交、发布包、双区域下载地址及校验值。
4. 通过 [投稿说明](CONTRIBUTING.md) 提交 PR 或 Issue，首次审核后收录。

作者可以在仓库添加 `echobird-plugin` 主题，帮助发现。主题不代表官方认证，也不会自动进入正式索引。

## 本地校验

需要 Node.js 24 或更高版本：

```sh
npm ci --ignore-scripts
npm run check
```

修改 `registry/` 后运行 `npm run build:index`，将生成的 `index.json` 与记录一起提交。校验不会安装插件依赖、执行插件脚本或访问投稿中的服务器。CI 在 Windows、macOS、Linux 检查同一套文件，`main` 全部通过后单向同步到 GitCode。

参见 [同步说明](docs/mirroring.md)。本仓库工具和模板使用 [MIT 许可证](LICENSE)，第三方插件保留各自许可证。
