import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Zero',
  description: 'Zero — A modular network proxy kernel written in Rust',
  lang: 'zh-CN',
  lastUpdated: true,

  themeConfig: {
    nav: [
      { text: '指南', link: '/guide/quickstart' },
      { text: '配置', link: '/project/config' },
      { text: '控制面', link: '/control-plane-api/' },
      { text: '参考', link: '/project/architecture' },
    ],

    sidebar: {
      '/guide/': [
        {
          text: '入门指南',
          items: [
            { text: '快速上手', link: '/guide/quickstart' },
            { text: 'GUI 集成', link: '/guide/gui-integration' },
          ],
        },
      ],

      '/project/': [
        {
          text: '配置',
          items: [
            { text: '配置规范', link: '/project/config' },
            { text: '模式与出站组', link: '/project/modes-and-groups' },
            { text: '引擎计划', link: '/project/engine-plan' },
            { text: 'API 类型', link: '/project/api' },
          ],
        },
        {
          text: '设计',
          items: [
            { text: '架构', link: '/project/architecture' },
            { text: '日志', link: '/project/logging' },
            { text: '生命周期', link: '/project/lifecycle' },
            { text: '定位', link: '/project/positioning' },
            { text: '目标', link: '/project/goals' },
            { text: '工具链', link: '/project/tooling' },
            { text: 'Panel 连接器', link: '/project/panel-node-connector' },
          ],
        },
      ],

      '/control-plane-api/': [
        {
          text: '控制面 API',
          items: [
            { text: '总览', link: '/control-plane-api/' },
            { text: '配置模型', link: '/control-plane-api/configuration' },
            { text: 'HTTP API', link: '/control-plane-api/http-api' },
            { text: 'IPC 协议', link: '/control-plane-api/ipc-protocol' },
            { text: '事件系统', link: '/control-plane-api/events' },
            { text: 'Flow Hooks', link: '/control-plane-api/hooks' },
            { text: 'Push Connector', link: '/control-plane-api/push-connector' },
            { text: 'CLI 命令', link: '/control-plane-api/cli' },
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
      prev: '上一页',
      next: '下一页',
    },

    lastUpdated: {
      text: '最后更新',
    },
  },
})
