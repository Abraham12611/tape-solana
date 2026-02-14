use solana_sdk::{
    signature::Keypair, 
    signer::Signer,
    transaction::Transaction,
    pubkey::Pubkey
};

use super::send_tx;
use litesvm::LiteSVM;

use spool_api::prelude::*;
use spool_api::instruction::program::{
    build_initialize_ix,
};
use spool_api::instruction::spool::{
    build_create_ix,
};

pub fn init_spool_program(svm: &mut LiteSVM, payer: &Keypair) {
    let payer_pk = payer.pubkey();

    let ix = build_initialize_ix(payer_pk);
    let blockhash = svm.latest_blockhash();
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);

    assert!(res.is_ok());
}

pub fn create_spool(svm: &mut LiteSVM, payer: &Keypair, spool_name: &str) -> (Pubkey, Pubkey) {
    let payer_pk = payer.pubkey();
    let (spool_address, _) = spool_pda(payer_pk, &to_name(spool_name));
    let (writer_address, _) = writer_pda(spool_address);

    let blockhash = svm.latest_blockhash();
    let ix = build_create_ix(payer_pk, spool_name);
    let tx = Transaction::new_signed_with_payer(&[ix], Some(&payer_pk), &[&payer], blockhash);
    let res = send_tx(svm, tx);
    assert!(res.is_ok());

    (spool_address, writer_address)
}
