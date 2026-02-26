use chia_bls::PublicKey;
use chia_inspect_core::recognize::recognize_puzzle_and_solution;
use chia_protocol::{Bytes32, Coin};
use chia_puzzle_types::{
    CoinProof, EveProof, Proof,
    cat::CatSolution,
    did::DidSolution,
    nft::{NftOwnershipLayerSolution, NftStateLayerSolution},
    singleton::SingletonSolution,
    standard::StandardSolution,
};
use chia_sdk_driver::{
    CatLayer, DidLayer, Layer, NftOwnershipLayer, NftStateLayer, RoyaltyTransferLayer,
    SingletonLayer, SpendContext, StandardLayer,
};
use clvmr::{NodePtr, serde::node_to_bytes};

fn node_bytes(ctx: &SpendContext, ptr: NodePtr) -> Vec<u8> {
    node_to_bytes(ctx, ptr).expect("node bytes")
}

fn wrapper_names(recognition: &chia_inspect_core::schema::PuzzleRecognition) -> Vec<String> {
    recognition
        .wrappers
        .iter()
        .map(|wrapper| wrapper.name.clone())
        .collect()
}

#[test]
fn recognizes_standard_layer() {
    let mut ctx = SpendContext::new();
    let layer = StandardLayer::new(PublicKey::default());
    let puzzle = layer.construct_puzzle(&mut ctx).expect("construct puzzle");
    let solution = layer
        .construct_solution(
            &mut ctx,
            StandardSolution {
                original_public_key: None,
                delegated_puzzle: NodePtr::NIL,
                solution: NodePtr::NIL,
            },
        )
        .expect("construct solution");

    let recognition = recognize_puzzle_and_solution(&node_bytes(&ctx, puzzle), &node_bytes(&ctx, solution));
    assert!(recognition.recognized);
    assert_eq!(wrapper_names(&recognition), vec!["standard_layer"]);
    assert!(recognition.parsed_solution.is_some());
}

#[test]
fn recognizes_cat_then_standard_layers() {
    let mut ctx = SpendContext::new();
    let standard_layer = StandardLayer::new(PublicKey::default());
    let cat_layer = CatLayer::new(Bytes32::new([7; 32]), standard_layer);
    let puzzle = cat_layer.construct_puzzle(&mut ctx).expect("construct puzzle");
    let solution = cat_layer
        .construct_solution(
            &mut ctx,
            CatSolution {
                inner_puzzle_solution: StandardSolution {
                    original_public_key: None,
                    delegated_puzzle: NodePtr::NIL,
                    solution: NodePtr::NIL,
                },
                lineage_proof: None,
                prev_coin_id: Bytes32::new([1; 32]),
                this_coin_info: Coin::new(Bytes32::new([2; 32]), Bytes32::new([3; 32]), 1),
                next_coin_proof: CoinProof {
                    parent_coin_info: Bytes32::new([4; 32]),
                    inner_puzzle_hash: Bytes32::new([5; 32]),
                    amount: 1,
                },
                prev_subtotal: 0,
                extra_delta: 0,
            },
        )
        .expect("construct solution");

    let recognition = recognize_puzzle_and_solution(&node_bytes(&ctx, puzzle), &node_bytes(&ctx, solution));
    assert!(recognition.recognized);
    assert_eq!(
        wrapper_names(&recognition),
        vec!["cat_layer", "standard_layer"]
    );
}

#[test]
fn recognizes_singleton_did_standard_layers() {
    let mut ctx = SpendContext::new();
    let launcher_id = Bytes32::new([9; 32]);
    let standard_layer = StandardLayer::new(PublicKey::default());
    let did_layer = DidLayer::new(launcher_id, None, 0, NodePtr::NIL, standard_layer);
    let singleton_layer = SingletonLayer::new(launcher_id, did_layer);

    let puzzle = singleton_layer
        .construct_puzzle(&mut ctx)
        .expect("construct puzzle");
    let solution = singleton_layer
        .construct_solution(
            &mut ctx,
            SingletonSolution {
                lineage_proof: Proof::Eve(EveProof {
                    parent_parent_coin_info: Bytes32::new([1; 32]),
                    parent_amount: 1,
                }),
                amount: 1,
                inner_solution: DidSolution::Spend(StandardSolution {
                    original_public_key: None,
                    delegated_puzzle: NodePtr::NIL,
                    solution: NodePtr::NIL,
                }),
            },
        )
        .expect("construct solution");

    let recognition = recognize_puzzle_and_solution(&node_bytes(&ctx, puzzle), &node_bytes(&ctx, solution));
    assert!(recognition.recognized);
    assert_eq!(
        wrapper_names(&recognition),
        vec!["singleton_layer", "did_layer", "standard_layer"]
    );
}

#[test]
fn recognizes_singleton_nft_state_ownership_standard_layers() {
    let mut ctx = SpendContext::new();
    let launcher_id = Bytes32::new([3; 32]);
    let standard_layer = StandardLayer::new(PublicKey::default());
    let transfer_layer = RoyaltyTransferLayer::new(launcher_id, Bytes32::new([4; 32]), 300);
    let ownership_layer =
        NftOwnershipLayer::new(Some(Bytes32::new([5; 32])), transfer_layer, standard_layer);
    let state_layer = NftStateLayer::new(NodePtr::NIL, Bytes32::new([6; 32]), ownership_layer);
    let singleton_layer = SingletonLayer::new(launcher_id, state_layer);

    let puzzle = singleton_layer
        .construct_puzzle(&mut ctx)
        .expect("construct puzzle");
    let solution = singleton_layer
        .construct_solution(
            &mut ctx,
            SingletonSolution {
                lineage_proof: Proof::Eve(EveProof {
                    parent_parent_coin_info: Bytes32::new([1; 32]),
                    parent_amount: 1,
                }),
                amount: 1,
                inner_solution: NftStateLayerSolution {
                    inner_solution: NftOwnershipLayerSolution {
                        inner_solution: StandardSolution {
                            original_public_key: None,
                            delegated_puzzle: NodePtr::NIL,
                            solution: NodePtr::NIL,
                        },
                    },
                },
            },
        )
        .expect("construct solution");

    let recognition = recognize_puzzle_and_solution(&node_bytes(&ctx, puzzle), &node_bytes(&ctx, solution));
    assert!(recognition.recognized);
    assert_eq!(
        wrapper_names(&recognition),
        vec![
            "singleton_layer",
            "nft_state_layer",
            "nft_ownership_layer",
            "standard_layer"
        ]
    );
}

#[test]
fn unknown_raw_puzzle_is_not_recognized() {
    let ctx = SpendContext::new();
    let puzzle = NodePtr::NIL;
    let solution = NodePtr::NIL;
    let recognition = recognize_puzzle_and_solution(&node_bytes(&ctx, puzzle), &node_bytes(&ctx, solution));
    assert!(!recognition.recognized);
    assert!(recognition.wrappers.is_empty());
}
