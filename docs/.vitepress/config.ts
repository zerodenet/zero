import { defineConfig } from 'vitepress'

type SidebarItem = {
  text: string
  link?: string
  items?: SidebarItem[]
  collapsed?: boolean
}

const group = (text: string, items: SidebarItem[], collapsed = true): SidebarItem => ({
  text,
  items,
  collapsed,
})

const page = (text: string, link: string): SidebarItem => ({ text, link })

const protocol = (
  text: string,
  route: string,
  details: Array<[string, string]>,
): SidebarItem => group(text, [
  page('概览', `/protocols/${route}/`),
  ...details.map(([label, name]) => page(label, `/protocols/${route}/${name}`)),
])

const guideSidebar: SidebarItem[] = [
  page('快速开始', '/guides/quickstart'),
  group('应用集成', [
    page('GUI 接入', '/guides/gui-integration'),
    page('配置错误处理', '/guides/config-failure-examples'),
  ]),
]

const projectSidebar: SidebarItem[] = [
  page('项目概览', '/project/'),
  group('使用与配置', [
    page('配置参考', '/project/config'),
    page('运行模式与出站组', '/project/modes-and-groups'),
    page('构建特性', '/project/features'),
  ], false),
  group('架构', [
    page('总体架构', '/project/architecture'),
    page('请求生命周期', '/project/lifecycle'),
    page('引擎计划', '/project/engine-plan'),
    page('协议能力模型', '/project/protocol-capabilities'),
  ]),
  group('格式与规范', [
    page('Zero 规则 IR v1', '/project/zero-rule-ir-v1'),
    page('ZRS 0.1 二进制格式', '/project/zrs-0.1'),
    page('ZRS 0.1 Golden Vector', '/project/zrs-0.1-golden'),
  ]),
  group('工程实践', [
    page('日志', '/project/logging'),
    page('工程规则', '/project/tooling'),
    page('发布边界', '/project/release-boundary'),
  ]),
  group('内部设计', [
    page('API 能力模型', '/project/api'),
    page('控制面规范', '/project/control-plane'),
    page('面板与节点连接器', '/project/panel-node-connector'),
  ]),
  group('项目背景', [
    page('项目定位', '/project/positioning'),
    page('项目目标', '/project/goals'),
  ]),
]

const protocolSidebar: SidebarItem[] = [
  page('协议概览', '/protocols/'),
  page('配置速查', '/protocols/configuration'),
  page('已知缺口', '/protocols/incomplete'),
  protocol('SOCKS5', 'socks5', [['入站', 'inbound'], ['出站', 'outbound'], ['公共编解码', 'shared']]),
  page('HTTP CONNECT', '/protocols/http/'),
  protocol('Mixed', 'mixed', [['入站', 'inbound'], ['实现边界', 'architecture']]),
  protocol('VLESS', 'vless', [['入站', 'inbound'], ['出站', 'outbound'], ['公共约定', 'shared']]),
  protocol('Hysteria2', 'hysteria2', [['入站', 'inbound'], ['出站', 'outbound'], ['公共约定', 'shared']]),
  protocol('Shadowsocks', 'shadowsocks', [['入站', 'inbound'], ['出站', 'outbound'], ['公共编解码', 'shared'], ['流加密', 'stream'], ['能力元数据', 'metadata']]),
  protocol('Trojan', 'trojan', [['入站', 'inbound'], ['出站', 'outbound'], ['公共编解码', 'shared'], ['能力元数据', 'metadata']]),
  protocol('Mieru', 'mieru', [['入站', 'inbound'], ['出站', 'outbound'], ['会话流程', 'flow']]),
  protocol('VMess', 'vmess', [['入站', 'inbound'], ['出站', 'outbound'], ['公共编解码', 'shared'], ['加密', 'crypto'], ['流封装', 'stream'], ['MUX', 'mux'], ['UDP', 'udp'], ['能力元数据', 'metadata']]),
]

const controlSidebar: SidebarItem[] = [
  page('控制与集成', '/control-plane-api/'),
  group('配置与命令', [
    page('控制面配置', '/control-plane-api/configuration'),
    page('CLI 命令', '/control-plane-api/cli'),
  ], false),
  group('接口契约', [
    page('HTTP JSON API', '/control-plane-api/http-api'),
    page('本地 IPC 协议', '/control-plane-api/ipc-protocol'),
    page('通用契约', '/control-plane-api/contract'),
    page('兼容性与破坏性变更', '/control-plane-api/breaking-changes'),
  ], false),
  group('事件与扩展', [
    page('事件目录', '/control-plane-api/events'),
    page('FlowHook', '/control-plane-api/hooks'),
    page('节点主动上报', '/control-plane-api/push-connector'),
  ]),
  group('集成指南', [
    page('GUI 接入', '/guides/gui-integration'),
    page('历史设计档案', '/control-plane/'),
  ]),
]

const controlArchiveSidebar: SidebarItem[] = [
  page('返回控制与集成', '/control-plane-api/'),
  page('设计档案说明', '/control-plane/'),
  group('历史方案', [
    page('实现路线图', '/control-plane/01-control-plane-roadmap'),
    page('API 端点草案', '/control-plane/02-api-endpoints'),
    page('HTTP 适配器设计', '/control-plane/03-http-adapter-design'),
    page('事件系统设计', '/control-plane/04-event-system'),
    page('认证与权限设计', '/control-plane/05-auth-and-permissions'),
    page('服务提供者集成', '/control-plane/06-service-provider-integration'),
    page('节点心跳与上报', '/control-plane/07-node-heartbeat-and-push'),
    page('性能与限流设计', '/control-plane/08-performance-and-rate-limiting'),
  ]),
]

export default defineConfig({
  title: 'Zero',
  description: 'Zero 模块化网络代理内核文档',
  lang: 'zh-CN',
  lastUpdated: true,

  themeConfig: {
    nav: [
      { text: '快速开始', link: '/guides/quickstart' },
      { text: '配置', link: '/project/config' },
      { text: '协议', link: '/protocols/' },
      {
        text: '控制与集成',
        items: [
          { text: '概览', link: '/control-plane-api/' },
          { text: 'HTTP JSON API', link: '/control-plane-api/http-api' },
          { text: '本地 IPC', link: '/control-plane-api/ipc-protocol' },
          { text: 'GUI 接入', link: '/guides/gui-integration' },
        ],
      },
      {
        text: '项目架构',
        items: [
          { text: '总体架构', link: '/project/architecture' },
          { text: '协议能力模型', link: '/project/protocol-capabilities' },
          { text: '工程规则', link: '/project/tooling' },
          { text: '格式与规范', link: '/project/zero-rule-ir-v1' },
        ],
      },
    ],

    sidebar: {
      '/guides/': guideSidebar,
      '/project/': projectSidebar,
      '/protocols/': protocolSidebar,
      '/control-plane-api/': controlSidebar,
      '/control-plane/': controlArchiveSidebar,
      '/testing/': [page('TUN 端到端测试', '/testing/tun-e2e')],
    },

    socialLinks: [{ icon: 'github', link: 'https://github.com/zerodenet/zero' }],
    search: { provider: 'local' },
    outline: { level: [2, 3], label: '本页目录' },
    docFooter: { prev: '上一页', next: '下一页' },
    lastUpdated: { text: '最后更新' },
    returnToTopLabel: '返回顶部',
    sidebarMenuLabel: '文档导航',
    darkModeSwitchLabel: '外观',
  },
})
