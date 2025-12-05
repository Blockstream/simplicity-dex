-- tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm maker
-- tex1p3tzxsj4cs64a6qwpcc68aev4xx38mcqmrya9r3587jy49sk40z3qk6d9el taker
-- tex1p9988q8kfq33m0y6wlsra683rur32k9vx58kqc6cceeks7tccu5yqhkjv7n dcd

-- FILLER_ASSET_ID
insert into outpoints (vout, owner_script_pubkey, asset_id, spent, tx_id)
values (1, 'tex1p3tzxsj4cs64a6qwpcc68aev4xx38mcqmrya9r3587jy49sk40z3qk6d9el',
        '2c3aa8ae0e199f9609e2e4b60a97a1f4b52c5d76d916b0a51e18ecded3d057b1', true,
        '1b993898b41c31cd88781e68ab9b2f6856c1c7d68921c74ef347412feac8ad6c'),
       (0, 'tex1p3tzxsj4cs64a6qwpcc68aev4xx38mcqmrya9r3587jy49sk40z3qk6d9el',
        '2c3aa8ae0e199f9609e2e4b60a97a1f4b52c5d76d916b0a51e18ecded3d057b1', false,
        '929d5526c41712d5cdf7b7406f645df229a62d24a1c76f63201d8e896da389ce'),
       (5, 'tex1p3tzxsj4cs64a6qwpcc68aev4xx38mcqmrya9r3587jy49sk40z3qk6d9el',
        '2c3aa8ae0e199f9609e2e4b60a97a1f4b52c5d76d916b0a51e18ecded3d057b1', true,
        'cd1e4aa43251fa1ebbf32e8f9b2e66358b746a32d6bcc0420a0a2e24e0393f4e'),
       (5, 'tex1p3tzxsj4cs64a6qwpcc68aev4xx38mcqmrya9r3587jy49sk40z3qk6d9el',
        '2c3aa8ae0e199f9609e2e4b60a97a1f4b52c5d76d916b0a51e18ecded3d057b1', false,
        '403c9bca043cbfb692bfad8ff7ea09634a838ae833f9a62aa043d2ffa4458387')
on conflict DO NOTHING;

-- GRANTOR_COLLATERAL_ASSET_ID
insert into outpoints (vout, owner_script_pubkey, asset_id, spent, tx_id)
values (6, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        'ba817efa46ffb5dd5b985d2c6657376ceaf748eedfda3f88e273260c18538d73', true,
        'cd1e4aa43251fa1ebbf32e8f9b2e66358b746a32d6bcc0420a0a2e24e0393f4e'),
       (6, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        'ba817efa46ffb5dd5b985d2c6657376ceaf748eedfda3f88e273260c18538d73', false,
        '403c9bca043cbfb692bfad8ff7ea09634a838ae833f9a62aa043d2ffa4458387'),
       (1, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        'ba817efa46ffb5dd5b985d2c6657376ceaf748eedfda3f88e273260c18538d73', false,
        '1eb9bed5e3954d0556de572ea12c73d6b4d7f62a4d11646cf1a07d943c2cb50e'),
       (6, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        'ba817efa46ffb5dd5b985d2c6657376ceaf748eedfda3f88e273260c18538d73', true,
        'e9fdd8eb41f7a87f101d9a2ae38a4b8584c892d9b7f22896696c79e805a30e95'),
       (1, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        'ba817efa46ffb5dd5b985d2c6657376ceaf748eedfda3f88e273260c18538d73', false,
        '929d5526c41712d5cdf7b7406f645df229a62d24a1c76f63201d8e896da389ce')
on conflict DO NOTHING;

-- GRANTOR_SETTLEMENT_ASSET_ID
insert into outpoints (vout, owner_script_pubkey, asset_id, spent, tx_id)
values (7, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        '82b7bba397cafbf1918cc8fee11aa636eba97ee4c88a6efe954b90e8a85806ea', false,
        'cd1e4aa43251fa1ebbf32e8f9b2e66358b746a32d6bcc0420a0a2e24e0393f4e'),
       (7, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        '82b7bba397cafbf1918cc8fee11aa636eba97ee4c88a6efe954b90e8a85806ea', true,
        '403c9bca043cbfb692bfad8ff7ea09634a838ae833f9a62aa043d2ffa4458387'),
       (1, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        '82b7bba397cafbf1918cc8fee11aa636eba97ee4c88a6efe954b90e8a85806ea', true,
        '1eb9bed5e3954d0556de572ea12c73d6b4d7f62a4d11646cf1a07d943c2cb50e'),
       (7, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        '82b7bba397cafbf1918cc8fee11aa636eba97ee4c88a6efe954b90e8a85806ea', false,
        'e9fdd8eb41f7a87f101d9a2ae38a4b8584c892d9b7f22896696c79e805a30e95'),
       (1, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        '82b7bba397cafbf1918cc8fee11aa636eba97ee4c88a6efe954b90e8a85806ea', true,
        '929d5526c41712d5cdf7b7406f645df229a62d24a1c76f63201d8e896da389ce')
on conflict DO NOTHING;

-- SETTLEMENT_ASSET_ID
insert into outpoints (vout, owner_script_pubkey, asset_id, spent, tx_id)
values (4, 'tex1p9988q8kfq33m0y6wlsra683rur32k9vx58kqc6cceeks7tccu5yqhkjv7n',
        '420561859e4217f0def578911bbf68d7d3f75d664b978de39083269994eecd4b', false,
        '403c9bca043cbfb692bfad8ff7ea09634a838ae833f9a62aa043d2ffa4458387'),
       (1, 'tex1p9988q8kfq33m0y6wlsra683rur32k9vx58kqc6cceeks7tccu5yqhkjv7n',
        '420561859e4217f0def578911bbf68d7d3f75d664b978de39083269994eecd4b', true,
        '1f3b3199bc5da2991d47fc9f30027393a09787308426a56abdcf336184213a22'),
       (3, 'tex1p3tzxsj4cs64a6qwpcc68aev4xx38mcqmrya9r3587jy49sk40z3qk6d9el',
        '420561859e4217f0def578911bbf68d7d3f75d664b978de39083269994eecd4b', false,
        '2655da67204d0c34b936b4a394e63ab84da421772d1e7779c1e257f7acb32d9b')
on conflict DO NOTHING;

-- COLLATERAL_ASSET_ID
insert into outpoints (vout, owner_script_pubkey, asset_id, spent, tx_id)
values (0, 'tex1p9988q8kfq33m0y6wlsra683rur32k9vx58kqc6cceeks7tccu5yqhkjv7n',
        '144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49', false,
        '1eb9bed5e3954d0556de572ea12c73d6b4d7f62a4d11646cf1a07d943c2cb50e'),
       (3, 'tex1p3tzxsj4cs64a6qwpcc68aev4xx38mcqmrya9r3587jy49sk40z3qk6d9el',
        '144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49', true,
        '2655da67204d0c34b936b4a394e63ab84da421772d1e7779c1e257f7acb32d9b'),
       (2, 'tex1p3tzxsj4cs64a6qwpcc68aev4xx38mcqmrya9r3587jy49sk40z3qk6d9el',
        '144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49', false,
        '2655da67204d0c34b936b4a394e63ab84da421772d1e7779c1e257f7acb32d9b'),
       (2, 'tex1pyzkfajdprt6gl6288z54c6m4lrg3vp32cajmqrh5kfaegydyrv0qtcg6lm',
        '144c654344aa716d6f3abcc1ca90e5641e4e2a7f633bc09fe3baf64585819a49', false,
        '0abf93fe8ae69f790e898c7b0a1f1b2ce2eb5e059d9ef6550fbd04ca8becf55d')
on conflict DO NOTHING;
