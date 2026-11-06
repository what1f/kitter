export type Lang = 'zh' | 'en';

const zh = {
  htmlLang: 'zh-CN',
  meta: {
    title: 'Kitter：集中管理 Agent Skills',
    description: '开源的桌面应用与命令行。集中管理 Agent Skills，按项目分配，查看每个 Agent 的加载情况与 token 占用。',
  },
  nav: { github: 'GitHub', download: '下载', switchTo: 'English', switchShort: 'EN', home: 'Kitter 首页', theme: '切换深浅色' },
  hero: {
    title: ['Agent Skills，', '集中管理。'],
    lede: '按项目分配 Skill，看清每个 Agent 实际加载了什么、占用多少 token。',
  },
  flow: {
    title: ['多个来源，', '按需分发。'],
    lede: '从 GitHub、skills.sh、Claude 插件或本地文件夹添加 Skill，也可以接管已有的安装，再装进项目或用户目录。',
    sources: { local: '本地文件夹', github: 'GitHub', skills: 'skills.sh', plugin: 'Claude 插件', existing: '已有安装' },
    user: '用户目录',
  },
  shots: { project: '项目', library: 'Skill', install: '安装' },
  drift: {
    title: ['一份源文件，', '所有项目同步。'],
    lede: '修改 Skill 后，引用它的项目随之更新。',
  },
  effective: {
    title: ['每个 Agent，', '加载了什么。'],
    lede: '汇总项目、上级目录、用户目录与插件中的 Skill，按 Agent 列出数量与 token 占用。',
    unit: 'tokens',
    skills: '个 Skill',
  },
  presets: {
    title: ['常用配置，', '存为预设。'],
    lede: '一组 Skill 加上 AGENTS.md 规则，一次装进项目，修改后同步更新。',
    file: [
      { t: '# kitter', k: 'h' },
      { t: '', k: 'n' },
      { t: '发版前先跑 just check。', k: 'n' },
      { t: '', k: 'n' },
      { t: '<!-- kitter:preset:frontend -->', k: 'm' },
      { t: '使用 pnpm，不用 npm', k: 'b' },
      { t: '组件放在 src/components', k: 'b' },
      { t: '提交前运行 pnpm check', k: 'b' },
      { t: '<!-- /kitter:preset:frontend -->', k: 'm' },
    ],
  },
  agents: {
    title: ['主流 Agent，', '内置支持。'],
    yours: '你的 Agent',
    lede: ['Claude Code、Codex、Cursor 等可直接使用。其他 Agent 可通过 ', 'JS 插件', ' 接入。'],
  },
  open: {
    title: '开源，原生，本地运行。',
    lede: '基于 Rust 与 GPUI 构建，提供桌面应用与命令行。Apache-2.0 协议。',
    star: '在 GitHub 上 Star',
    download: '下载',
  },
  footer: { releases: '版本记录' },
  scopes: { project: '项目', parent: '上级目录', user: '用户级', plugin: '插件' },
};

type Dict = typeof zh;

const en: Dict = {
  htmlLang: 'en',
  meta: {
    title: 'Kitter: manage agent skills in one place',
    description: 'An open-source desktop app and CLI. Manage agent skills in one place, assign them per project, and see what each agent loads and what it costs in tokens.',
  },
  nav: { github: 'GitHub', download: 'Download', switchTo: '中文', switchShort: '中', home: 'Kitter home', theme: 'Toggle dark mode' },
  hero: {
    title: ['Agent skills,', 'in one place.'],
    lede: 'Assign skills per project. See what each agent loads and how many tokens it takes.',
  },
  flow: {
    title: ['Many sources.', 'Installed where needed.'],
    lede: 'Add skills from GitHub, skills.sh, Claude plugins or a local folder, or adopt ones already installed. Then put them in projects or your user directory.',
    sources: { local: 'Local folder', github: 'GitHub', skills: 'skills.sh', plugin: 'Claude plugins', existing: 'Existing installs' },
    user: 'User directory',
  },
  shots: { project: 'Projects', library: 'Skills', install: 'Install' },
  drift: {
    title: ['One source.', 'Every project in sync.'],
    lede: 'Edit a skill and every project that uses it updates.',
  },
  effective: {
    title: ['What each agent', 'actually loads.'],
    lede: 'Skills from the project, parent folders, user directory and plugins, listed per agent with their token cost.',
    unit: 'tokens',
    skills: 'skills',
  },
  presets: {
    title: ['Save your setup', 'as a preset.'],
    lede: 'Bundle skills and AGENTS.md instructions into a preset and install it in any project. When the preset changes, sync it.',
    file: [
      { t: '# kitter', k: 'h' },
      { t: '', k: 'n' },
      { t: 'Run just check before every release.', k: 'n' },
      { t: '', k: 'n' },
      { t: '<!-- kitter:preset:frontend -->', k: 'm' },
      { t: 'Use pnpm, not npm', k: 'b' },
      { t: 'Components live in src/components', k: 'b' },
      { t: 'Run pnpm check before committing', k: 'b' },
      { t: '<!-- /kitter:preset:frontend -->', k: 'm' },
    ],
  },
  agents: {
    title: ['Built-in support for', 'major agents.'],
    yours: 'Your agent',
    lede: ['Claude Code, Codex, Cursor and others work directly. Connect any other agent with a ', 'JS plugin', '.'],
  },
  open: {
    title: 'Open source. Native. Runs locally.',
    lede: 'Built with Rust and GPUI. Desktop app and CLI. Apache-2.0.',
    star: 'Star on GitHub',
    download: 'Download',
  },
  footer: { releases: 'Releases' },
  scopes: { project: 'Project', parent: 'Parent folders', user: 'User', plugin: 'Plugins' },
};

export const dicts: Record<Lang, Dict> = { zh, en };
export const pathFor = (lang: Lang) => (lang === 'en' ? '/' : '/zh/');

/* Illustrative project for the effective-skills panel. */
type Scope = 'project' | 'parent' | 'user' | 'plugin';
const pool: Record<string, { scope: Scope; tokens: number }> = {
  'code-review': { scope: 'project', tokens: 112 },
  'frontend-design': { scope: 'project', tokens: 148 },
  'release-notes': { scope: 'project', tokens: 96 },
  'writing-for-agents': { scope: 'parent', tokens: 86 },
  'ui-review': { scope: 'user', tokens: 64 },
  defuddle: { scope: 'user', tokens: 41 },
  imagegen: { scope: 'user', tokens: 73 },
  'ego-browser': { scope: 'user', tokens: 188 },
  'agent-browser': { scope: 'user', tokens: 342 },
  slides: { scope: 'user', tokens: 268 },
  'skill-creator': { scope: 'plugin', tokens: 97 },
  'deep-research': { scope: 'plugin', tokens: 214 },
  pdf: { scope: 'plugin', tokens: 132 },
  docx: { scope: 'plugin', tokens: 156 },
};
const pick = (names: string[]) => names.map((name) => ({ name, ...pool[name] }));
export const project = { name: 'kitter', path: '~/projects/kitter' };
export const loads = [
  { id: 'claude', name: 'Claude Code', skills: pick(['code-review', 'frontend-design', 'release-notes', 'writing-for-agents', 'ui-review', 'defuddle', 'skill-creator', 'deep-research', 'pdf', 'docx']) },
  { id: 'codex', name: 'Codex', skills: pick(['code-review', 'frontend-design', 'release-notes', 'writing-for-agents', 'ui-review', 'imagegen', 'ego-browser']) },
  { id: 'cursor', name: 'Cursor', skills: pick(['code-review', 'frontend-design', 'release-notes', 'writing-for-agents']) },
  { id: 'opencode', name: 'OpenCode', skills: pick(['code-review', 'frontend-design', 'release-notes', 'writing-for-agents', 'ui-review', 'defuddle', 'imagegen', 'ego-browser', 'agent-browser', 'slides', 'skill-creator', 'deep-research', 'pdf', 'docx']) },
  { id: 'antigravity', name: 'Antigravity', skills: pick(['code-review', 'frontend-design', 'ui-review', 'ego-browser']) },
  { id: 'pi', name: 'Pi', skills: pick(['code-review', 'frontend-design', 'release-notes', 'imagegen']) },
];
/* Same thresholds as the app: above this a context starts to feel heavy. */
export const TOKEN_WARNING = 2000;
