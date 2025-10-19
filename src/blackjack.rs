use rand::prelude::SliceRandom;

pub fn sort_cards(deck: &mut Vec<u8>) -> [u8;4] {
    println!("Aleatorizar 2 pares");

    deck.shuffle(&mut rand::rng());
    let pares: [u8; 4] = [deck[0], deck[1], deck[2], deck[3]];

    for i in 0..3{
    deck.swap_remove(i);
    };

    pares
}

pub fn create_room(player1_id: &str) -> String{
    let room_id = format!("room_{}", player1_id);
    room_id
}