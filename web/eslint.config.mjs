import nextPlugin from '@next/eslint-plugin-next';

export default [
  {
    ignores: ['.next/**', '.next-dev/**', 'node_modules/**', 'public/sw.js', 'public/workbox-*.js'],
  },
  nextPlugin.flatConfig.coreWebVitals,
];
