# 自定义主题

[English](themes.md)

主题决定桌面端的颜色、表面质感、圆角、字体、字号、控件高度和图标。颜色模式（浅色 / 深色）单独选择，所以一个主题要同时描述两种模式。

## 位置

主题放在 `<Kitter 数据目录>/themes`（设置了 `KITTER_HOME` 时为 `$KITTER_HOME/themes`）。在设置页打开「风格」菜单，选择「打开主题文件夹」即可找到。

主题可以是单个文件，也可以是一个主题包文件夹：

```text
themes/
├── ocean.json            # 单文件主题，id 为 "ocean"
└── pixel/                # 主题包，id 为 "pixel"
    ├── theme.json
    ├── icons/            # 可选：替换内置图标的 SVG
    │   ├── folder.svg
    │   ├── search.svg
    │   └── color/        # 可选：保留原色的彩色 SVG 图标
    │       └── folder.svg
    ├── images/           # 可选：纹理和边框用的 .png、.jpg 图片
    │   ├── grain.png
    │   └── bevel.png
    └── fonts/            # 可选：主题用到的 .ttf、.otf 或 .ttc 字体
        └── PixelFont.ttf
```

主题包和文件同名时以主题包为准。`standard` 和 `solid_glass` 是内置主题的 id，同名的用户主题会被忽略。

修改主题后，在菜单里重新选一次即可生效。

## 命令行

```bash
kitter theme                                 # 列出主题和主题目录，* 标记当前主题
kitter theme create ocean --from solid_glass # 新建主题包，写入某个主题的全部取值
kitter theme check ocean                     # 列出被忽略的键、无效的值和未知的图标
kitter theme check ./pixel                   # 检查任意位置的主题文件或主题包
kitter theme use ocean                       # 切换主题，重启桌面端后生效
kitter theme show solid_glass                # 以 theme.json 的形式输出主题的全部取值
kitter theme icons --theme pixel             # 列出可替换的图标，* 标记该主题替换了的
kitter theme add ./pixel                     # 把主题文件或主题包复制进主题目录
kitter theme remove ocean
```

`add` 可以用 `--id` 换一个 id 安装，用 `--force` 覆盖已有的同名主题。

## 格式

```json
{
  "name": "Ocean",
  "extends": "solid_glass",
  "shape": { "radius_xl": 16 },
  "light": {
    "colors": { "accent": "#0a7cff", "focus": "accent" }
  },
  "dark": {
    "colors": { "accent": "#5fb0ff" }
  }
}
```

- `name`：菜单里显示的名称，默认为文件名或文件夹名。
- `extends`：基于哪个主题，可以是内置主题，也可以是另一个用户主题，默认 `standard`。没写的部分都来自它，包括它的图标和字体。
- `font`、`text`、`shape`、`size`：两种模式共用的取值。
- `materials`：各类表面的质感，两种模式共用。
- `light` / `dark`：各模式的 `colors`，以及可选的 `materials`，用来在该模式下覆盖共用的材质。

只写想改的部分。无效的值和未知的键会被忽略，`kitter theme check` 会列出它们。JSON 解析失败的主题不会出现在菜单里。

### 颜色

颜色写成 `#rrggbb` 或 `#rrggbbaa`，也可以写同一模式里另一个颜色的名字，例如 `"focus": "accent"`。

可以用小写字母、数字和 `_` 自己起名添加颜色，在材质或其他颜色里引用。内置主题也是这么做的：Standard 定义了 `shadow` 和 `shadow_deep`，Solid Glass 的高光和细线也都是这样定义的。

| 键 | 用途 |
| --- | --- |
| `window`、`window_border` | 窗口背景和外框 |
| `base` | 侧栏右侧的主内容区 |
| `sidebar` | 侧栏 |
| `pane` | 主内容区里的列表和文件树面板 |
| `surface` | 卡片 |
| `elevated` | 菜单、弹出层和对话框 |
| `avatar` | 头像和相互叠放的图块 |
| `raised`、`hover`、`selected` | 凸起、悬停和选中的填充 |
| `border`、`border_strong` | 普通边框和强调边框 |
| `text`、`sidebar_text`、`secondary`、`muted` | 正文、侧栏、次要和弱化文字 |
| `accent`、`focus` | 强调色和获得焦点的输入框边框 |
| `primary`、`on_primary` | 主按钮填充及其文字 |
| `danger`、`on_danger`、`danger_soft` | 危险色、危险按钮上的文字、危险提示的底色 |
| `success`、`warning` | 状态色 |
| `overlay` | 对话框背后的遮罩 |
| `dialog_base`、`dialog_pane`、`dialog_surface` | 对话框里的 `base`、`pane` 和 `surface`，默认与这三个相同 |
| `switch_track`、`switch_thumb` | 开关的颜色，不写则保持默认 |

### 材质

材质决定某一类表面的质感，最多五个键：

- `fill`：替换表面背景的颜色，也可以是渐变：`"linear-gradient(135deg, primary 0%, primary_deep 100%)"`。角度和 CSS 一致（`0deg` 朝上，`90deg` 朝右），默认 `180deg`，即从上到下。渐变有两个颜色，位置默认为 `0%` 和 `100%`。
- `border`：外框颜色，宽度由 `shape.border` 决定。
- `shadows`：阴影列表，写法同 CSS 的 `box-shadow`：`"[inset] x y [blur [spread]] 颜色"`，单位为像素。颜色可以是色值或颜色名。空列表表示去掉阴影。
- `texture`：用 `images/` 里的图片平铺在填充上，比如纸纹、扫描线。
- `frame`：用 `images/` 里的图片按九宫格画在外框上，比如立体斜面、手绘边框。

`texture` 和 `frame` 写 `"none"` 可以去掉基础主题设置的值。

```json
"materials": {
  "pane": { "border": "edge", "shadows": ["inset 0 1 0 0 rim", "0 12 32 -14 shade"] },
  "control": { "fill": "surface", "shadows": ["2 2 0 0 #000000"] },
  "content": { "texture": { "image": "grain.png", "size": 128, "opacity": 0.6 } },
  "dialog": { "frame": { "image": "bevel.png", "slice": 4, "width": 8, "pixelated": true } }
}
```

纹理可以直接写文件名，也可以写成对象：

| 键 | 含义 |
| --- | --- |
| `image` | `images/` 里的文件名 |
| `size` | 一块图案的宽度，单位为像素，默认为图片本身的宽度 |
| `opacity` | 0 到 1，默认 1 |
| `pixelated` | 为 `true` 时放大保持像素硬边，适合像素画 |

边框图片会被切成 3×3 九块：四个角原样绘制，四条边沿边拉伸，中间一块默认不画，设置 `fill` 后才画：

| 键 | 含义 |
| --- | --- |
| `image` | `images/` 里的文件名 |
| `slice` | 从图片各边切进去的距离，单位为图片像素：一个数、`[上下, 左右]` 或 `[上, 右, 下, 左]` |
| `width` | 每条边绘制的宽度，单位为像素，写法同上，默认等于 `slice` |
| `repeat` | 为 `true` 时边沿平铺而不是拉伸 |
| `fill` | 为 `true` 时把中间一块画成表面背景，取代 `fill` |
| `pixelated` | 为 `true` 时放大保持像素硬边 |

边框图片会取代外框颜色，外框宽度仍然决定内边距，需要时把 `shape.border` 调成和图片边缘一致。文本输入框只使用 `field` 的颜色和阴影，不支持 `texture` 和 `frame`。

| 材质 | 用途 |
| --- | --- |
| `pane` | 卡片等静止的面板 |
| `popover` | 菜单、通知和拖拽预览 |
| `dialog` | 对话框 |
| `control` | 次要按钮、徽标和标签 |
| `state` | 选中或悬停的行、标签页、分段 |
| `primary` | 主按钮 |
| `field` | 输入框、下拉框和选择器 |
| `well` | 分段控件的凹槽 |
| `window` | 最底层的窗口 |
| `sidebar` | 侧栏 |
| `content` | 侧栏右侧的主内容区 |
| `column` | 主内容区里的列表和文件树面板 |

`window`、`sidebar`、`content`、`column` 是底面，适合放铺满整块区域的纹理和渐变。

`inset` 阴影和 CSS 一样画在外框以内。桌面端没有背景模糊，外阴影也会透过半透明的填充显示出来，所以带外阴影的表面要配不透明的 `fill`。

### 字体

| 键 | 用途 |
| --- | --- |
| `ui` | 界面字体 |
| `mono` | 技能名、路径和代码的字体 |
| `medium` | 标签和强调文字的字重，100–900 |
| `semibold` | 标题的字重，100–900 |

可以用系统已安装的字体，也可以把字体文件放进主题包的 `fonts/`，再把 `ui` 或 `mono` 设成它的字体族名。

### 字号

`caption`、`small`、`body`、`emphasis`、`title`、`display`，单位为像素。Standard 依次为 11、12、13、14、16、20。

### 形状

| 键 | 用途 |
| --- | --- |
| `radius_sm` | 徽标、标签、菜单项和提示 |
| `radius_md` | 按钮、输入框、下拉框、标签页和列表行 |
| `radius_lg` | 菜单、弹出层和通知 |
| `radius_xl` | 卡片 |
| `radius_modal` | 对话框 |
| `radius_pill` | 头像、圆点等圆形元素；设为 `0` 即变成方形 |
| `border` | 卡片、控件、输入框和弹出层的外框宽度 |
| `divider` | 行与区块之间分隔线的宽度 |

### 尺寸

高度（像素）：`control_sm`（工具栏、菜单项、标签页）、`control_md`（表单控件、对话框按钮、侧栏项）、`row`、`row_double`、`row_setting`（列表行和设置行）、`pane_header`、`card_header`、`file_bar`、`tab_bar`、`segment`、`chip`、`badge`。

另有 `avatar`（直径），以及 `icon`、`icon_sm`、`icon_xs`（图标尺寸）。

所有数值的范围是 0 到 1000。

## 图标

主题包可以在 `icons/` 里放 `<名字>.svg` 来替换内置图标。名字可以用 `kitter theme icons` 查看：

`check`、`chevron-down`、`chevron-right`、`chevron-up`、`clock-3`、`crown`、`download`、`ellipsis`、`file`、`folder`、`folder-open`、`hand`、`hash`、`house`、`layers`、`loader-circle`、`package`、`pencil`、`plus`、`rotate-cw`、`search`、`settings`、`sidebar`、`sparkle`、`trash`、`triangle-alert`、`x`。

图标会用周围文字的颜色绘制，只有形状起作用。请使用正方形的 SVG。没有替换的图标保持内置样式。

想保留图标本身的颜色，就把它放进 `icons/color/`。彩色图标原样绘制，不跟随文字颜色，记得在浅色和深色模式下都检查一遍。同一个主题包里两种都有时，以彩色图标为准。
