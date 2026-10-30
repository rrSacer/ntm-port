# Reatores R5: pesquisa/pequeno (fora do escopo)

Decisão: **não portar**. A leitura de 2026-10-30 do fonte atual do NTM (`4f5924e`) mostra que a família inteira é legada:

| Item | Evidência no fonte |
| --- | --- |
| `reactor_research` (bloco `machine_reactor_small`) | `@Deprecated public static Block reactor_research` em `ModBlocks`; o `.lang` o nomeia "Research Reactor (LEGACY)"; `TileEntityReactorResearch` também é `@Deprecated` |
| `machine_controller` | `@Deprecated`, `setCreativeTab(null)` (fora do criativo); só existe para o reator de pesquisa |
| `plate_fuel_u233/u235/mox/pu239/sa326/ra226be/pu238be` | `@Deprecated`, `setCreativeTab(null)`; as placas de resíduo também |
| `reactor_sensor` | O único consumidor é `TileEntityReactorControl` (o `machine_controller` acima); a receita está atrás de `!GeneralConfig.enable528` e depende de itens ainda ausentes |

As placas só têm uso no reator de pesquisa, e nenhuma delas tem obtenção no fonte fora de receitas de construção do próprio reator; portá-las seria inventar fluxo de jogo para peças ocultas. O levantamento inicial (`docs/reactors-survey.md`) já avisava "não inventar obtenção onde o fonte não oferece". Não há IDs, tabelas nem receitas novas neste lote, e nenhuma mudança de tabela para o responsável pela lógica.

Reabrir apenas se o NTM remover o `@Deprecated` ou se um lote de ferramentas/sensores (`reactor_sensor` é um item `consumableTab` ativo no registro, mas inútil sem o controlador) for pedido explicitamente. As peças existentes `reactor_core` e `dysfunctional_reactor` não pertencem a esta família.
