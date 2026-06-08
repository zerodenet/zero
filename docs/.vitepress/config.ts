import { defineConfig } from 'vitepress'

export default defineConfig({
  title: 'Zero',
  description: 'Zero - A modular network proxy kernel written in Rust',
  lang: 'zh-CN',
  lastUpdated: true,

  themeConfig: {
    nav: [
      { text: '指南', link: '/guides/quickstart' },
      { text: '配置', link: '/project/config' },
      { text: '协议', link: '/protocols/' },
      { text: '控制面', link: '/control-plane-api/' },
      { text: '参考', link: '/project/architecture' },
    ],

    sidebar: {
      '/guides/': [
        {
          text: '入门指南',
          items: [
            { text: '快速上手', link: '/guides/quickstart' },
            { text: 'GUI 集成', link: '/guides/gui-integration' },
          ],
        },
      ],

      '/protocols/': [
        {
          text: '协议实现',
          items: [
            { text: '协议总览', link: '/protocols/' },
            { text: '配置速查', link: '/protocols/configuration' },
            { text: 'Shadowsocks', link: '/protocols/shadowsocks' },
            { text: '未完成项', link: '/protocols/incomplete' },
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
            { text: '编译参数', link: '/project/features' },
            { text: '日志', link: '/project/logging' },
            { text: '生命周期', link: '/project/lifecycle' },
            { text: '控制面规范', link: '/project/control-plane' },
            { text: '定位', link: '/project/positioning' },
            { text: '目标', link: '/project/goals' },
            { text: '工具链', link: '/project/tooling' },
            { text: 'Panel 连接器', link: '/project/panel-node-connector' },
            { text: '协议能力矩阵', link: '/project/protocol-capabilities' },
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
