# Reatores R2: RBMK

Entram 105 IDs, levando o catálogo a 2.940 (498 blocos e 2.442 itens). Só conteúdo: as máquinas entram como Inert no registro nativo. Foram reaproveitados o catálogo de metadata, os tradutores de receitas/hazards, o visual-gen e o probe de conteúdo do R1.

| Grupo | IDs (prefixo `ntm:`) |
| --- | --- |
| Colunas (18) | `rbmk_rod`, `_rod_mod`, `_rod_reasim`, `_rod_reasim_mod`, `_control`, `_control_mod`, `_control_auto`, `_control_reasim`, `_control_reasim_auto`, `_blank`, `_boiler`, `_reflector`, `_absorber`, `_moderator`, `_outgasser`, `_storage`, `_cooler`, `_heater` |
| Console e periféricos (15) | `rbmk_console`, `_crane_console`, `_autoloader`, `_loader`, `_steam_inlet`, `_steam_outlet`; painéis `_display_blank`, `_display`, `_key_pad`, `_gauge`, `_numitron`, `_graph`, `_lever`, `_indicator`, `_terminal` |
| Destroços (4) | `pribris`, `pribris_burning`, `pribris_radiating`, `pribris_digamma` (cubos simples, sem block entity) |
| Barras (33) | `rbmk_fuel_<tipo>`: ueu, meu, heu233, heu235, uzh, thmeu, lep, mep, hep (a variável `rbmk_fuel_hep239` tem o nome registrado `rbmk_fuel_hep`), hep241, lea, mea, hea241, hea242, men, hen, mox, les, mes, hes, leaus, heaus, po210be, ra226be, pu238be, balefire_gold, flashlead, balefire, zfb_bismuth, zfb_pu241, zfb_am_mix, drx, test |
| Pellets (32) | `rbmk_pellet_<tipo>` (os mesmos tipos, `hep239` com o nome completo, sem `test`) |
| Outros (3) | `rbmk_lid`, `rbmk_lid_glass`, `rbmk_tool` |

As 33 máquinas usam o template `beehive` (facing horizontal, honey_level 0..5), com `auto: false` no visual.json; a camada visual e o registro Inert ficam com as outras frentes. Texturas `_side`/`_top` originais por coluna; as nomeações vêm do `.lang` (a `rbmk_fuel_test` não tem entrada no fonte e usa o rótulo "RBMK Fuel Rod (THE VOICES)", do `fullName` do fonte). A pt_BR cai para o inglês quando o fonte não traduz.

**Pellet.** O fonte usa uma id por tipo com metadata 0..9: grau 0..4 (novo até esgotado) mais envenenamento por xenônio a partir do 5; cinco tipos (po210be, ra226be, balefire_gold, flashlead, balefire) não têm xenônio. Foi portado como 1 id por tipo; o estado é dado nativo do item (o `rbmk_pellet_*` é produto de reciclagem). Decisão para revisar se a SILEX ou o resultado de `RBMKFuelCraftingHandler` precisarem de IDs por estado.

**Vida da barra.** Rendimento, xenônio e calores são NBT do fonte; não usa `minecraft:max_damage`. A tabela original está em `content/machine-recipes/reactors_rbmk.json`: 33 barras (rendimento, função de queima e de depleção, calor, ponto de fusão, difusão, xenônio, coeficiente de calor, tipos de nêutron, tinta), as fórmulas das nove funções de queima e das cinco de depleção, o contrato do NBT, o estado do pellet, a regra barra→pellet e os 25 dials de `RBMKDials` com os padrões. Colunas, console, guindaste, autoloader, painéis, fluxo, calor, fusão/destroços, desmontagem, SILEX e fuel pool são trabalho nativo posterior.

**Receitas originais** (+56 arquivos; vanilla 1.030 no total): barras `RodRecipes.addRBMKRod` (casca `rbmk_fuel_empty` + oito billets, as duas sobrecargas, validadas contra o corpo do helper; as outras duas barras, `drx` e `test`, não têm receita viável: o `drx` pede `particle_digamma`, fora do catálogo), as colunas, o console, o guindaste, as tampas, a ferramenta e os periféricos de `CraftingManager`. Os blocos sob `if(!GeneralConfig.enable528)` entram porque o padrão é falso; o `else` (as variantes ReaSim 528: `rbmk_control_reasim` e `_auto`) fica de fora, como no jogo com a configuração padrão. A regra é restrita a essa condição exata. Montadora 457 (+`ass.rbmk`, `ass.rbmkautoloader`); bigorna 136 construções (+13 do RBMK); estas duas tabelas e os hazards/buckets foram aprovados pelo responsável da lógica. Ficam como lacuna as receitas com itens ainda fora do catálogo (painéis de rádio, `crate_steel` etc.), a `RBMKFuelCraftingHandler` (dinâmica), a SILEX e a reciclagem.

**Hazards.** +105 identidades, 60 radioativas. O extrator lê agora `registerRBMKRod`, `registerRBMKPellet` e `registerRBMK`: exporta radiação fresca, `blinding` e `digamma`, e registra como lacuna a interpolação por depleção e o HOT do núcleo. `rbmk_fuel_balefire` e `rbmk_fuel_drx` chegam a 1.200.000.000 mrad/s (bit 30), dentro do u32 do canal de radiação. Não se mudou nenhuma taxa antiga.

**Gerador.** `generate.registration` termina a instrução no primeiro `;` fora de comentário `//` (as cadeias de barras têm `;` em comentários); regenerar não alterou nenhuma identidade anterior.

**Prova.** `verify-bedrock-processing.py --profile reactors-r2` coloca as 33 máquinas em dois níveis, os 4 destroços e entrega as 105 identidades pelo inventário no binário nativo, com o detector de rejeição. Resultado: 105 identidades, 419 conferências, sem `reject` nem `panic` no log do cliente. Validação: 192 testes por seed 1/42, `validate-bulk` e `recipe-gen/validate` verdes. Pacote `ntm.hmod` com SHA-256 `21b014b2f70b2cc1eeebc8babc25bc057bba93dc82cc31eac3c7a2b8ff086f37`, aprovado no check do upstream e do fork.
