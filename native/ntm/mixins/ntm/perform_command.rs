fn ntm_perform_bomb_command(game:&mut crate::game::Game<'_>,player:&horizon_client::mod_api::NativeServerPlayer,command:&str,ran:&mut bool){
    if !*ran && crate::game::ntm_glue::command(game,player,command){*ran=true;}
}
