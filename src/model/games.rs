
#[derive(Clone, Default, Debug)]
pub struct GameMetadata{
    pub game_id : String,
    pub white_elo: u32,
    pub black_elo: u32,
    pub time_control: String,
    pub termination: String,
    pub results: String

}


#[derive(Clone, Default, Debug)]
pub struct PgnGame{
    pub metadata: GameMetadata,
    pub pgn_text: String
}
