# Petróleo, lote C: vapor e processamento

Este lote acrescenta sete blocos: `machine_boiler`, `machine_industrial_boiler`, `machine_catalytic_cracker`, `machine_catalytic_reformer`, `machine_hydrotreater`, `machine_vacuum_distill` e `machine_coker`. Todos usam o contrato beehive/facing/honey_level 0..5, nomes en_us/pt_br, loot próprio e texturas NTM opacas como fallback. O visual-gen conserva a propriedade de seus modelos, telas e multiblocos.

O catálogo chega a **2.653 IDs**: 389 blocos, 2.264 itens e 52 máquinas. Não houve fluido novo nem worldgen. Os hazards, as 97 taxas/27 bits de radiação e as oito abas foram regenerados; o datapack conserva 905 receitas vanilla.

## Tabelas completas e construção

`tools/content-gen/petroleum_processing.py` e `boiler_recipes.py`, chamados pelo pipeline de máquinas, exportam schema 1 na ordem da fonte:

| JSON | Fonte | Receitas exportadas | Skipped |
| --- | --- | ---: | ---: |
| cracking.json | CrackingRecipes | 12 | 0 |
| reforming.json | ReformingRecipes | 9 | 0 |
| hydrotreating.json | HydrotreatingRecipes | 6 | 0 |
| vacuum_refinery.json | VacuumRefineryRecipes | 2 | 0 |
| coker.json | CokerRecipes | 33 | 0 |
| boiler.json | Fluids FT_Heatable + tiles | 6 ativas, 18 passos conservados | 0 |

Todas as construções padrão resolveram: cinco na montadora (`ass.crackingtower`, `ass.coker`, `ass.vaccumrefinery`, `ass.reformer`, `ass.hydrotreater`) e as duas caldeiras na bigorna. A grafia `vaccumrefinery` é a da fonte. A montadora passa a 424 receitas; a seção de construção da bigorna passa de 103/154 para **105 exportadas / 152 skipped**. A receita `ass.arcfurnace` ainda depende de machine_transformer, previsto para D; nenhuma ponte foi inventada.

## Energia, frequência, pressão e catalisadores

A escolha das cinco tabelas é pelo fluido do tanque de entrada. O esquema conserva inputs e outputs de itens/fluidos, fontes, índices e labels. Os campos adicionais abaixo foram combinados com o agente de lógica antes da exportação. **O generic atual precisa de adaptação explícita para executá-los**; a entrega de conteúdo mantém as máquinas Inert até a autorização da lógica.

O cracker consome 100 mB do fluido e 200 mB de vapor por operação, produz suas frações e 2 mB de spentsteam. Não usa HE. Executa **até duas operações a cada cinco ticks**: `max_operations_per_interval:2`, `interval_ticks:5`. A saída NONE/zero da rota de querosene foi omitida; a ausência não vira um fluido real no port.

Reformer e hidrotratador exigem um catalytic_converter no slot 10, sem consumir ou desgastar. O campo `catalysts:[{item,count,consumed:false,slot,source}]` fica separado de inputItems. A energia é debitada instantaneamente quando a operação consegue ocorrer, sem ser dividida por ticks:

| Máquina | HE por operação | Intervalo |
| --- | ---: | ---: |
| Reformer | 20.000 | 1 tick |
| Hidrotratador | 20.000 | 2 ticks |
| Destilador a vácuo | 10.000 | 1 tick |

Essas receitas têm `energy_per_operation_he`, `interval_ticks` e `energy_draw:"instant_on_success"`. `duration` registra o intervalo; `power:0` impede que um leitor antigo interprete a energia por operação como HE/t contínuo. Os três buffers são de 1.000.000 HE. `inputFluidPressures` é paralelo a inputFluids: hidrotratamento usa pressão 1 no hidrogênio; destilação a vácuo usa pressão 2 na entrada. Cada tanque e os parâmetros explícitos conservam a fonte em machine_config.

## Coqueificadora térmica

A coqueificadora é aquecida por um IHeatSource abaixo; não foi convertida para HE ou para uma duração fixa. O buffer é de **100.000 TU**, diffusion 0,25, e cada operação exige **20.000 unidades de progresso térmico**. Por tick, burn=floor(heat/100), progress+=burn e heat-=burn. Se progress alcança processTime, executa no máximo uma operação, subtrai processTime e conserva o restante. A transferência e a perda térmica ficam documentadas em machine_config, e cada receita declara heat_required_tu.

Os 24 registerAuto usam 820.000 TU por coque; WOODOIL usa 340.000 TU por carvão vegetal. O cálculo considera **max(calor final de FT_Flammable, energia final de FT_Combustible)**, penalidade 1, truncagem e os mesmos degraus de arredondamento Java. O subproduto é max(10,mB/10), com divisão inteira. As oito receitas explícitas também entram, incluindo lama, mercúrio, bitume, slop e vitriol. Todos os requisitos cabem no tanque padrão de 16.000 mB. O helper de energia passou a oferecer esse máximo sem alterar o resultado padrão usado pela pirólise e pela solidificação.

## Caldeiras

O exportador conserva **15 declarações FT_Heatable, com 18 passos**. Só o primeiro passo de cada fluido com eficiência BOILER positiva é ativo na caldeira, conforme getFirstStep(). Há seis conversões ativas: ar, água e as quatro variantes de óleo. Os três passos superiores da água e os nove passos de outros trocadores/refrigrantes permanecem em heating_steps como inativos; não viram escolhas indevidas de receita.

Água: 1 mB → 100 mB de steam, 200 TU. Óleo, óleo dessulfurizado e suas variantes craqueadas: 1 mB → 1 mB da variante quente, 10 TU. Ar: 1 mB → 1 mB de airblast, 5 TU. A quantidade convertida por tick depende de insumo, espaço e calor; não existe duração fixa inventada.

Caldeira pequena: buffer de 3.200.000 TU e entrada de 16.000 mB. Industrial: 12.800.000 TU e entrada de 64.000 mB. Ambas têm diffusion 0,1; a capacidade de saída acompanha a razão produzida/requerida do primeiro passo. Estão documentados o aquecimento externo, o resfriamento, o aquecimento ambiental de TomSaveData e a explosão por saída bloqueada da pequena; a industrial não tem essa explosão em tryConvert. Esses comportamentos são dados para a futura lógica, não uma alegação de que já funcionam no port.

## Verificação

Passaram **107 testes Python em cada seed, PYTHONHASHSEED=1 e 42**, incluindo 12 testes do lote C; passaram `validate-bulk.py` e `tools/recipe-gen/validate.py`. A suíte cobre as chamadas/ordem, pressões, catalisador não consumido, energia instantânea, duas operações do cracker, cálculo e progresso térmico, passos ativos das caldeiras, construção dos sete blocos, referências e regeneração em diretório novo. TMPDIR dos testes aponta para disco em `.agent-tmp`.

Build/check API 0.2 passou no disco: **5.750.325 bytes**, SHA-256 `dcff0610b45412a83a4b84be12a8bdbbd8477fe96a3af6295e4a6bf21d201a35`, pacote em `../.agent-tmp/ntm-petroleum-c-content-0hycvvn5/target/horizon-mod/ntm.hmod`. Evidências: `content-gen-evidence/petroleum-c/`. O agente de lógica registrou os sete Inert em `516a8c0` e confirmou `cargo test --workspace` e clippy `-D warnings` no HEAD isolado por git archive em disco. A integração e o conteúdo já foram publicados no push conjunto do visual. Nenhuma instalação ou mundo real foi alterado.
