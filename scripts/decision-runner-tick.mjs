#!/usr/bin/env node

import { runTick, withErrorHandler } from './runner-framework.mjs';
import * as lib from './decision-runner-lib.mjs';

withErrorHandler(() =>
  runTick({
    name: 'decision-runner',
    envKey: 'DECISION_RUNNER_ENABLED',
    limitEnvKey: 'DECISION_RUNNER_LIMIT',
    defaultLimit: 100,
    endpoint: '/decisions/runner/tick',
    lib,
  }),
);
