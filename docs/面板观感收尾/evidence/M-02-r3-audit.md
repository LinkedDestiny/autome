# M-02 r3 独立复验（审计 #2）

## 验收命令
`npx vitest run` 契约所列 8 个文件：8 文件 / 87 用例通过、0 失败（预期 ≥ 86）。

## 负向对照（均改 `app/styles/app.css` 或 TSX，跑完立即还原，`git status` 干净）
| 注入 | 结果 |
|---|---|
| 基础规则选择器去掉 `select` | `ui-base`「七类控件…」失败 |
| 复选/单选规则写 `appearance: auto` | `ui-base`「七类控件…」「全站样式不恢复浏览器原生外观」失败 |
| 勾的颜色写 `#fff` | `styles`「不散写颜色」失败 |
| `task-notes.tsx` 的 `<input>` 加 `type="date"` | `ui-base`「全站 TSX 控件类型…」失败 |

注：首次注入用错文件（该文件没有 input，注入为 0 处），已识别并换到有 input 的文件重做，结论以重做为准。

## 自建判别检查
- 全站 TSX `<input type=…>` 统计：checkbox 5、file 1、hidden 21、number 2、radio 1、search 1，均在覆盖集合内。
- 页面规则里没有对 input/select/textarea 重复声明 1px 边框或圆角像素的残留。
- `fieldset`、`legend` 的 `:where` 基础规则存在（app.css 20–21 行）；`:where(...):focus-visible` 黄色焦点环存在（22 行）。
- 构造的检查结果：全部符合设计 A-Q2，无缺陷。

## 未取证
浏览器实际渲染（复选框勾、下拉箭头目视）需监听端口，本轮未跑；由 M-03 起的 e2e 抽屉用例与门禁覆盖，不阻塞本里程碑。

## 结论
通过。
