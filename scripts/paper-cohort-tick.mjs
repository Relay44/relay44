#!/usr/bin/env node

import { runTick, withErrorHandler } from './runner-framework.mjs';
import * as lib from './paper-cohort-lib.mjs';

withErrorHandler(() =>
  runTick({
    name: 'paper-cohort',
    envKey: 'PAPER_COHORT_ENABLED',
    defaultLimit: 200,
    endpoint: '/external/agents/runner/tick',
    lib,
  }),
);
