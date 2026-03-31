#!/usr/bin/env node

import { runTick, withErrorHandler } from './runner-framework.mjs';
import * as lib from './external-runner-lib.mjs';

withErrorHandler(() =>
  runTick({
    name: 'external-runner',
    envKey: 'EXTERNAL_RUNNER_ENABLED',
    limitEnvKey: 'EXTERNAL_RUNNER_LIMIT',
    defaultLimit: 200,
    endpoint: '/external/agents/runner/tick',
    lib,
  }),
);
