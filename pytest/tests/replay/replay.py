from account import Account
from collections import OrderedDict
from key import Key
from messages.tx import tx_schema, SignedTransaction
from messages.crypto import crypto_schema, Signature
from messages.bridge import bridge_schema
from serializer import BinarySerializer
import mocknet_helpers

import argparse
import json
import os

def generate_new_key():
    return Key.implicit_account()

def save_genesis_with_new_key_pair(genesis_path, key_pair, output_path):
    NODE0_DIR = os.path.join(output_path, 'node0/')
    if not os.path.exists(NODE0_DIR):
        os.makedirs(NODE0_DIR)
    with open(genesis_path) as fin:
        genesis = json.load(fin)

    new_key = key_pair.pk.split(':')[1] if ':' in key_pair.pk else key_pair.pk
    for validator in genesis['validators']:
        validator['public_key'] = new_key
    for record in genesis['records']:
        if 'AccessKey' in record:
            record['AccessKey']['public_key'] = new_key
    with open(os.path.join(NODE0_DIR, 'genesis.json'), 'w') as fout:
        json.dump(genesis, fout, indent=2)

    key_json = dict()
    key_json['account_id'] = key_pair.account_id
    key_json['public_key'] = key_pair.pk
    key_json['secret_key'] = key_pair.sk
    with open(os.path.join(NODE0_DIR, 'node_key.json'), 'w') as fout:
        json.dump(key_json, fout, indent=2)
    with open(os.path.join(NODE0_DIR, 'validator_key.json'), 'w') as fout:
        json.dump(key_json, fout, indent=2)

def prompt_to_launch_localnet():
    input('Please launch your localnet node now and press enter to continue...')

def send_resigned_transactions(tx_path, key_pair):
    LOCALHOST = '127.0.0.1'
    base_block_hash = mocknet_helpers.get_latest_block_hash(addr=LOCALHOST)
    nonce = mocknet_helpers.get_nonce_for_key(key_pair, addr=LOCALHOST)
    my_account = Account(key_pair,
                         init_nonce=nonce,
                         base_block_hash=base_block_hash,
                         rpc_infos=[(LOCALHOST, "3030")])

    schema = dict(tx_schema + crypto_schema + bridge_schema)
    replaced_public_key = str(key_pair.decoded_pk())
    with open(tx_path) as fin:
        txs = json.load(fin, object_pairs_hook=OrderedDict)
    for original_signed_tx in txs:
        my_account.prep_tx()
        tx = original_signed_tx['transaction']
        tx.publicKey = replaced_public_key
        tx.nonce = my_account.nonce
        msg = BinarySerializer(schema).serialize(tx)
        hash_ = hashlib.sha256(msg).digest()
        signature = Signature()
        signature.keyType = 0
        signature.data = key_pair.sign_bytes(hash_)
        resigned_tx = SignedTransaction()
        resigned_tx.transaction = tx
        resigned_tx.signature = signature
        my_account.send_tx(BinarySerializer(schema).serialize(resigned_tx))

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description='Setup replay')
    parser.add_argument('--tx-json', type=str, required=True, help="Path of tx history json")
    parser.add_argument('--genesis', type=str, required=True, help="Path of genesis")
    parser.add_argument('--output-dir', type=str, required=True, help="Path of the new home directory")
    args = parser.parse_args()

    key_pair = generate_new_key()
    save_genesis_with_new_key_pair(args.genesis, key_pair, args.output_dir)
    prompt_to_launch_localnet()
    send_resigned_transactions(args.tx_json, key_pair)
