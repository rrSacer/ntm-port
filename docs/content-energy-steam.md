# Energia, lote 2: vapor, turbinas e condensadores

Conteúdo novo: **cinco máquinas** (`machine_steam_engine`, `machine_industrial_turbine`, `machine_chungus`, `machine_condenser`, `machine_condenser_powered`) e **duas engrenagens** (`gear_large__iron`, `gear_large__steel`). Total: **2.686 IDs, 418 blocos, 2.268 itens e 67 máquinas**. As máquinas usam beehive/facing/honey_level 0..5, textura opaca de fallback, loot próprio e nomes en_us/pt_br do NTM. O visual-gen conserva a propriedade de sua sobreposição. A large_turbine Deprecated continua fora.

ItemGear não tem habilidade/durabilidade própria: metadado 0 usa ferro nas receitas, metadado 1 é steel; ambos usam a textura gear_large do fonte e recebem nomes distintos. Os IDs mantêm esses metadados e empilhamento 64. Não foi criada uma variante wood. A ação de reparo de Stirling pertence à lógica da máquina e não é alegada nesta entrega.

## Construções destravadas

| Estação original | Conteúdo |
|---|---|
| Bancada | Engrenagem de ferro: oito placas de ferro + um lingote de cobre; engrenagem de aço: oito placas de aço + um lingote de titânio. Condensador simples conforme CraftingManager.java:844. São três receitas bulk novas. |
| Bigorna tier 2 | Motor a vapor, agora com gear_large de ferro resolvido. Entram também as desmontagens originais das duas engrenagens. Construção passa **110/147→113/144** (emitidas/skipped). |
| Montadora | `ass.iturbine`, `ass.leviturbine`, `ass.hpcondenser`; total passa **430/201→433/198**. |

O condensador avançado estava bloqueado pelo parser em `Fluids.LUBRICANT.getDict(1000)`, e não por um fluido ausente. `FluidType.java:183–185` define o nome do OreDict por volume e stringId; `FluidContainerRegistry.java:69–98` registra os recipientes. O resolver agora aceita os recipientes dinâmicos efetivamente registrados no fonte que existem no catálogo, com fluido e volume exatos: **canister_full, fluid_tank_full e fluid_tank_lead_full de lubricant**, quantidade quatro na receita normal. Não aceita barril 16.000 mB, pack 32.000 mB ou recipientes externos como substitutos de 1.000 mB. O teste também exige falha para volume sem recipiente. Não houve substituição por fluido de tanque nem receita temporária.

## Contrato algorítmico

`content/machine-recipes/steam.json`, schema 1, tem **cinco machine_config, quatro turbine_operations e uma condensação**; recipes/skipped estão vazios. O contrato não apresenta uma falsa receita generic com duração ou potência fixa. As quatro operações são derivadas do FT_Coolable com TURBINE positivo em `content/fluid_traits.json`, já extraído no lote 1. Nenhum fluido ou trait foi inventado.

| Máquina | Regra preservada do NTM |
|---|---|
| Steam engine | Tanques 2.000/20 mB; somente steam→spentsteam; eficiência 0,85. Consome operações inteiras permitidas pelos tanques e reinicia powerBuffer a cada tick. |
| Industrial turbine | Tanques iniciais 750.000/3.000.000 mB; usa até 20% do fill por tick, com ceil antes de dividir pelo amountReq. Eficiência 1,0. A energia entra no volante; spin=energia/50.000.000 e a descarga depende de spin/maxPower, sem teto de spin inventado. maxPower é atualizado quando há geração; preserva-se a inércia quando o fluxo para. |
| Chungus | Tanques 1.000.000.000/1.000.000.000 mB; consumo 100%, eficiência 0,85, sem volante industrial nem resize de tanques ao mudar modo. |
| Condensador simples | Tanques 100/100 mB; spentsteam→water 1:1, sem HE, limitado ao fill e espaço livre. |
| Condensador powered | Tanques 1.000.000/1.000.000 mB; buffer 10.000.000 HE; custo 10 HE/mB. A conversão é integral, com pré-condição power≥convert×10×0,95, depois desconto integral e clamp em zero. Não se inventou conversão parcial pelo orçamento de HE. |

A industrial e a chungus conservam os quatro modos steam→spentsteam, hotsteam→steam, superhotsteam→hotsteam e ultrahotsteam→superhotsteam. Somente a industrial divide os dois tanques por dez nas transições para vapor mais denso e multiplica por mil ao voltar do ultrahotsteam para steam.

**Semântica inteira do fonte:** em TurbineBase, `ops * trait.heatEnergy` é multiplicação Java int32 antes da eficiência double. Uma chungus muito cheia no modo ultrahotsteam pode transbordar esse intermediário. O JSON registra a ordem e o wrap da fonte explicitamente; não foi introduzida correção silenciosa nem afirmado que o runtime já a implementa. Modos, portas, resize, volante, animação e comportamento térmico/energético dependem da futura lógica. Este lote não a aceita como funcional.

## Verificação

Passaram **152 testes Python em cada seed (1 e 42)** e os validadores bulk/recipe. Os oito testes novos verificam os cinco estados suportados e a exclusão da turbina Deprecated, os metadados/crafts das engrenagens, as estações originais, alternativas exatas do lubrificante e falha por volume errado, proporções/eficiências de vapor, diferenças de consumo/volante/resize, tolerância do condensador powered, ausência de power/duration falsos e regeneração em diretório novo.

Hazards, abas, nomes e cobertura foram regenerados. Build/check passou: **6.359.204 bytes**, SHA-256 `f229bf5fa246a2459f4bfdeb3b4b128a81681a4a5b15e8f6c5c9656e7b777b8a`. A prova obrigatória no horizon-patched, com Xvfb, HORIZON_NO_AUDIO, options em zero, mute.py e perfil temporário em disco, passou **68 conferências**: /setblock dos cinco IDs em facing=north/honey_level=0 e 5 e /give dos cinco blocos e das duas engrenagens, conferidos no inventário. O detector não encontrou rejeição do conteúdo. O agente de lógica registrou os cinco Inert em `002c833`, sobre as tabelas regeneradas de `9cb1501`; isso não representa aceitação da geração funcional de HE ou condensação. Nenhum worldgen mudou neste lote. As evidências ficam em `content-gen-evidence/energy-steam/`.

O agente de lógica conferiu uma exportação limpa com `cargo test --workspace` e `cargo clippy --workspace -- -D warnings`, ambos verdes após `9cb1501`/`002c833`. Os modelos automáticos pendentes pertencem ao visual-gen e não foram incluídos nos commits deste lote.
