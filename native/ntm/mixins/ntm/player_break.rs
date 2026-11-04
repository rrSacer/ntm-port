fn ntm_player_break(_value:(),game:&mut crate::game::Game<'_>,pos:[i32;3],state:horizon_world::registry::BlockStateId,canceled:bool) { crate::game::ntm_glue::player_break(game,pos,state,canceled); }
