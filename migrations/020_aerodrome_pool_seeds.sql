-- Seed major Aerodrome Slipstream (CL) pools on Base
INSERT INTO aerodrome_pools (id, pool_address, token0, token1, fee, tick_spacing, token0_symbol, token1_symbol, token0_decimals, token1_decimals, is_slipstream, active)
VALUES
    ('weth-usdc-cl100', '0xb2cc224c1c9feE385f8ad6a55b4d94E92359DC59',
     '0x4200000000000000000000000000000000000006', '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
     500, 100, 'WETH', 'USDC', 18, 6, TRUE, TRUE),

    ('weth-cbbtc-cl100', '0x22aee3699b6a0fed71490c103bd4e5f3309891d5',
     '0x4200000000000000000000000000000000000006', '0xcbB7C0000aB88B473b1f5aFd9ef808440eed33Bf',
     100, 100, 'WETH', 'cbBTC', 18, 8, TRUE, TRUE),

    ('cbbtc-usdc-cl200', '0x4e962bb3889bf030368f56810a9c96b83cb3e778',
     '0xcbB7C0000aB88B473b1f5aFd9ef808440eed33Bf', '0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913',
     500, 200, 'cbBTC', 'USDC', 8, 6, TRUE, TRUE)
ON CONFLICT (id) DO NOTHING;
