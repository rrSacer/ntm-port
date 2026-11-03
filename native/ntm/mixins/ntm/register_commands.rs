fn ntm_register_bomb_command(commands:&mut Vec<horizon_client::mod_api::NativeCommand>){
    commands.push(horizon_client::mod_api::NativeCommand::op("ntmbomba [args:text]",2));
}
