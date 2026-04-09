-- Seed distribution markets
-- Run via: psql $DATABASE_URL -f scripts/seed_distribution_markets.sql
-- All markets start as status=0 (active), mu = midpoint, sigma = range/6

INSERT INTO distribution_markets
  (id, question, description, category, status, outcome_min, outcome_max, outcome_unit,
   liquidity_param, market_mu, market_sigma, collateral_token, total_collateral,
   total_volume, volume_24h, fee_bps, resolver, use_oracle, trading_end, resolution_deadline, created_at)
VALUES
  -- Crypto
  ('dist-btc-eoy-2026',
   'What will the price of BTC be on Dec 31, 2026?',
   'Resolves to the CoinGecko BTC/USD spot price at 23:59 UTC on December 31, 2026.',
   'crypto', 0, 20000, 500000, 'USD',
   500, 260000, 80000, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
   0, 0, 0, 100, NULL, FALSE,
   '2026-12-31T23:59:00Z', '2027-01-07T23:59:00Z', NOW()),

  ('dist-eth-eoy-2026',
   'What will the price of ETH be on Dec 31, 2026?',
   'Resolves to the CoinGecko ETH/USD spot price at 23:59 UTC on December 31, 2026.',
   'crypto', 0, 500, 25000, 'USD',
   500, 12750, 4083.33, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
   0, 0, 0, 100, NULL, FALSE,
   '2026-12-31T23:59:00Z', '2027-01-07T23:59:00Z', NOW()),

  ('dist-sol-eoy-2026',
   'What will the price of SOL be on Dec 31, 2026?',
   'Resolves to the CoinGecko SOL/USD spot price at 23:59 UTC on December 31, 2026.',
   'crypto', 0, 10, 2000, 'USD',
   500, 1005, 331.67, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
   0, 0, 0, 100, NULL, FALSE,
   '2026-12-31T23:59:00Z', '2027-01-07T23:59:00Z', NOW()),

  ('dist-btc-dominance-q3-2026',
   'What will BTC dominance be on Sep 30, 2026?',
   'Resolves to CoinGecko BTC dominance percentage at 23:59 UTC on September 30, 2026.',
   'crypto', 0, 30, 80, '%',
   300, 55, 8.33, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
   0, 0, 0, 100, NULL, FALSE,
   '2026-09-30T23:59:00Z', '2026-10-07T23:59:00Z', NOW()),

  ('dist-total-crypto-mcap-eoy-2026',
   'What will total crypto market cap be on Dec 31, 2026?',
   'Resolves to the CoinGecko total market cap in trillions USD at 23:59 UTC on December 31, 2026.',
   'crypto', 0, 1, 20, 'T USD',
   300, 10.5, 3.17, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
   0, 0, 0, 100, NULL, FALSE,
   '2026-12-31T23:59:00Z', '2027-01-07T23:59:00Z', NOW()),

  -- Finance
  ('dist-sp500-eoy-2026',
   'What will the S&P 500 close at on Dec 31, 2026?',
   'Resolves to the S&P 500 closing value on the last trading day of 2026.',
   'finance', 0, 3000, 8000, 'pts',
   400, 5500, 833.33, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
   0, 0, 0, 100, NULL, FALSE,
   '2026-12-31T23:59:00Z', '2027-01-07T23:59:00Z', NOW()),

  ('dist-fed-rate-eoy-2026',
   'What will the Fed Funds Rate be on Dec 31, 2026?',
   'Resolves to the upper bound of the Federal Reserve target rate on December 31, 2026.',
   'finance', 0, 0, 8, '%',
   300, 4, 1.33, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
   0, 0, 0, 100, NULL, FALSE,
   '2026-12-31T23:59:00Z', '2027-01-07T23:59:00Z', NOW()),

  -- Technology
  ('dist-nvidia-eoy-2026',
   'What will NVIDIA stock price be on Dec 31, 2026?',
   'Resolves to the NVDA closing price on the last trading day of 2026.',
   'technology', 0, 30, 300, 'USD',
   400, 165, 45, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
   0, 0, 0, 100, NULL, FALSE,
   '2026-12-31T23:59:00Z', '2027-01-07T23:59:00Z', NOW()),

  ('dist-global-ai-revenue-2026',
   'What will global AI market revenue be in 2026?',
   'Resolves to Statista or IDC reported global AI market revenue for calendar year 2026, in billions USD.',
   'technology', 0, 100, 2000, 'B USD',
   300, 1050, 316.67, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
   0, 0, 0, 100, NULL, FALSE,
   '2027-03-31T23:59:00Z', '2027-06-30T23:59:00Z', NOW()),

  -- Science
  ('dist-global-temp-anomaly-2026',
   'What will the 2026 global temperature anomaly be?',
   'Resolves to NASA GISS annual global temperature anomaly for 2026 relative to 1951-1980 baseline, in degrees Celsius.',
   'science', 0, 0.5, 2.5, 'C',
   300, 1.5, 0.33, '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
   0, 0, 0, 100, NULL, FALSE,
   '2027-01-31T23:59:00Z', '2027-03-31T23:59:00Z', NOW())

ON CONFLICT (id) DO NOTHING;
