import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Zero',
  description: 'Zero - A modular network proxy kernel written in Rust',
  lang: 'zh-CN',
  lastUpdated: true,

  themeConfig: {
    nav: [
      { text: 'Guides', link: '/guides/quickstart' },
      { text: 'Config', link: '/project/config' },
      { text: 'Protocols', link: '/protocols/' },
      { text: 'Control Plane', link: '/control-plane-api/' },
      { text: 'Project', link: '/project/architecture' },
    ],

    sidebar: {
      '/guides/': [
        {
          text: 'Guides',
          items: [
            { text: 'Quickstart', link: '/guides/quickstart' },
            { text: 'GUI Integration', link: '/guides/gui-integration' },
          ],
        },
      ],

      '/protocols/': [
        {
          text: 'Protocol Tracking',
          items: [
            { text: 'Overview', link: '/protocols/' },
            { text: 'Configuration', link: '/protocols/configuration' },
            { text: 'SOCKS5', link: '/protocols/socks5' },
            { text: 'HTTP CONNECT', link: '/protocols/http-connect' },
            { text: 'Mixed', link: '/protocols/mixed' },
            { text: 'VLESS', link: '/protocols/vless' },
            { text: 'Shadowsocks', link: '/protocols/shadowsocks' },
            { text: 'Trojan', link: '/protocols/trojan' },
            { text: 'Hysteria2', link: '/protocols/hysteria2' },
            { text: 'Mieru', link: '/protocols/mieru' },
            { text: 'VMess', link: '/protocols/vmess' },
            { text: 'Incomplete', link: '/protocols/incomplete' },
          ],
        },
      ],

      '/project/': [
        {
          text: 'Config',
          items: [
            { text: 'Config Spec', link: '/project/config' },
            { text: 'Modes And Groups', link: '/project/modes-and-groups' },
            { text: 'Engine Plan', link: '/project/engine-plan' },
            { text: 'API Types', link: '/project/api' },
          ],
        },
        {
          text: 'Design',
          items: [
            { text: 'Architecture', link: '/project/architecture' },
            { text: 'Features', link: '/project/features' },
            { text: 'Logging', link: '/project/logging' },
            { text: 'Lifecycle', link: '/project/lifecycle' },
            { text: 'Control Plane', link: '/project/control-plane' },
            { text: 'Positioning', link: '/project/positioning' },
            { text: 'Goals', link: '/project/goals' },
            { text: 'Tooling', link: '/project/tooling' },
            { text: 'Panel Connector', link: '/project/panel-node-connector' },
            { text: 'Protocol Capabilities', link: '/project/protocol-capabilities' },
          ],
        },
      ],

      '/control-plane-api/': [
        {
          text: 'Control Plane API',
          items: [
            { text: 'Overview', link: '/control-plane-api/' },
            { text: 'Configuration Model', link: '/control-plane-api/configuration' },
            { text: 'HTTP API', link: '/control-plane-api/http-api' },
            { text: 'IPC Protocol', link: '/control-plane-api/ipc-protocol' },
            { text: 'Events', link: '/control-plane-api/events' },
            { text: 'Flow Hooks', link: '/control-plane-api/hooks' },
            { text: 'Push Connector', link: '/control-plane-api/push-connector' },
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
      label: 'On This Page',
    },

    docFooter: {
      prev: 'Previous',
      next: 'Next',
    },

    lastUpdated: {
      text: 'Last Updated',
    },
  },
})
