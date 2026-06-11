import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Zero',
  description: 'Zero - 模块化网络代理内核（Rust）',
  lang: 'zh-CN',
  lastUpdated: true,

  themeConfig: {
    nav: [
      { text: '指南', link: '/guides/quickstart' },
      { text: '配置', link: '/project/config' },
      { text: '协议', link: '/protocols/' },
      { text: '控制面', link: '/control-plane-api/' },
      { text: '项目', link: '/project/architecture' },
    ],

    sidebar: {
      '/guides/': [
        {
          text: '指南',
          items: [
            { text: '快速上手', link: '/guides/quickstart' },
            { text: 'GUI 对接', link: '/guides/gui-integration' },
            { text: '配置失败示例', link: '/guides/config-failure-examples' },
          ],
        },
      ],

      '/protocols/': [
        {
          text: '协议追踪',
          items: [
            { text: '概览', link: '/protocols/' },
            { text: '配置速查', link: '/protocols/configuration' },
            { text: '未完成项', link: '/protocols/incomplete' },
          ],
        },
        {
          text: '协议详情',
          items: [
            { text: 'SOCKS5', link: '/protocols/socks5/' },
            { text: 'HTTP CONNECT', link: '/protocols/http-connect/' },
            { text: 'Mixed', link: '/protocols/mixed/' },
            { text: 'VLESS', link: '/protocols/vless/' },
            { text: 'Shadowsocks', link: '/protocols/shadowsocks/' },
            { text: 'Trojan', link: '/protocols/trojan/' },
            { text: 'Hysteria2', link: '/protocols/hysteria2/' },
            { text: 'Mieru', link: '/protocols/mieru/' },
            { text: 'VMess', link: '/protocols/vmess/' },
          ],
        },
      ],

      '/project/': [
        {
          text: '配置与运行时',
          items: [
            { text: '配置规范', link: '/project/config' },
            { text: '模式与分组', link: '/project/modes-and-groups' },
            { text: '引擎计划', link: '/project/engine-plan' },
            { text: 'API 类型', link: '/project/api' },
          ],
        },
        {
          text: '架构设计',
          items: [
            { text: '架构', link: '/project/architecture' },
            { text: '构建特性', link: '/project/features' },
            { text: '日志', link: '/project/logging' },
            { text: '生命周期', link: '/project/lifecycle' },
            { text: '控制面', link: '/project/control-plane' },
            { text: '项目定位', link: '/project/positioning' },
            { text: '项目目标', link: '/project/goals' },
            { text: '工程规则', link: '/project/tooling' },
            { text: '面板连接器', link: '/project/panel-node-connector' },
            { text: '协议能力', link: '/project/protocol-capabilities' },
          ],
        },
      ],

      '/control-plane-api/': [
        {
          text: '控制面 API',
          items: [
            { text: '概览', link: '/control-plane-api/' },
            { text: '配置模型', link: '/control-plane-api/configuration' },
            { text: 'HTTP API', link: '/control-plane-api/http-api' },
            { text: 'IPC 协议', link: '/control-plane-api/ipc-protocol' },
            { text: '事件', link: '/control-plane-api/events' },
            { text: '流钩子', link: '/control-plane-api/hooks' },
            { text: '推送连接器', link: '/control-plane-api/push-connector' },
            { text: 'CLI', link: '/control-plane-api/cli' },
          ],
        },
      ],
    },

    socialLinks: [
      { icon: 'github', link: 'https://github.com/zerodenet/zero' },
    ],

    search: {
      provider: 'local',
    },

    outline: {
      level: [2, 3],
      label: '本页目录',
    },

    docFooter: {
      prev: '上一篇',
      next: '下一篇',
    },

    lastUpdated: {
      text: '最后更新',
    },
  },
})
