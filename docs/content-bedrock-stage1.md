# Cadeia de bedrock: conteúdo da etapa 1

Esta frente acrescenta **14 IDs**: três blocos, o minério bruto e dez brocas. O catálogo passa de 2.564 para **2.578 IDs** (367 blocos, 2.211 itens; 36 máquinas). Não altera a lógica Rust: densidades, seleção por chunk, energia, ácido, extração, `Stack.custom` e slopper pertencem ao agente de máquinas; OBJ, telas e multiblocos pertencem ao visual-gen.

## Blocos e itens

| ID | Contrato |
|---|---|
| `ntm:ore_bedrock` | Bloco simples, template bedrock, hardness −1, resistência 3.600.000, sem block entity e sem loot. Não consta em `machines`. |
| `ntm:machine_excavator` | Beehive; facing + honey_level 0..5; casing NTM de aço como fallback. |
| `ntm:machine_ore_slopper` | Mesmo contrato de máquina. |
| `ntm:bedrock_ore_base` | Minério bruto; densidades em custom_data pela lógica, sem receita artificial. |
| `ntm:drillbit__<tipo>` | Dez itens de pilha 1, metadata e atributos do EnumDrillType fonte. |

O depósito usa a textura **vanilla bedrock**, como o substrato de `BlockBedrockOreTE`, sem empacotar uma cópia do asset vanilla. `texture_reference` permite referenciá-la diretamente no gerador. A sobreposição dinâmica do minério original não é reproduzida nesta etapa. Mantém-se um item opcional do depósito para comandos/criativo; ele não tem função de máquina. As duas máquinas conservam 24 estados no fallback; o visual-gen pode reivindicar seus arquivos pelo manifesto próprio.

As brocas são steel, steel_diamond, hss, hss_diamond, desh, desh_diamond, tcalloy, tcalloy_diamond, ferro e ferro_diamond. `ids.json.entries[].drill` e `bedrock.json.drillbits` conservam speed, tier, fortune, vein e silk. Não se inventa desgaste: esse enum não declara durabilidade. Nomes en_us/pt_br e ícones vêm do NTM.

## bedrock.json e receitas

`content/machine-recipes/bedrock.json`, schema 1, é gerado de `ItemBedrockOreNew`, `ItemDrillbit`, `BedrockOre`, `WorldConfig`, `CentrifugeRecipes`, `AssemblyMachineRecipes` e OreDict/Mats. O JSON compacto retém referências fonte e estas tabelas:

| Campo | Contagem / regra |
|---|---|
| types | 6, na ordem light, heavy, rare, actinide, nonmetal, crystal; primários e três grupos de subprodutos. |
| grades | 26, na ordem do enum; `items` mapeia os seis tipos aos 156 IDs existentes; metadata = `(grade << 4) | type`. |
| nether | Glowstone dust ×4, powder_fire ×4, quartz ×4; pesos padrão 100/50/100; tier 1. |
| drillbits | 10; parâmetros exatos da fonte. |
| centrifuge | 96 receitas, uma por entrada (16 graus × seis tipos), com até quatro saídas. |
| assembly | 12 receitas: escavadora, dez brocas e processador; também disponíveis no assembly.json principal. |
| fragment_packing | 45 conversões de bancada 9→1, com chave OreDict escolhida e saída. |
| skipped / missing_fragments | Vazios na fonte e catálogo atuais. Identidades ausentes produzem omissões explícitas. |

Cada receita da centrífuga tem `inputItems:[{ids,count}]` e `outputItems:[[id,count],...]`. BASE e BASE_ROASTED rendem PRIMARY + gravel; BASE_WASHED rende duas pilhas separadas de PRIMARY + gravel. As demais linhas da fonte também foram extraídas, mesmo quando dependem de etapas posteriores. `extract` usa **min(ceil(amount × k), 64)**. Saídas repetidas são mantidas em slots separados; não há entradas duplicadas. O bruto `bedrock_ore_base` não ganha receita de centrífuga: ele passa pelo slopper.

Há **45** IDs `bedrock_ore_fragment__*`, todos com saídas resolvidas; a contagem de 54 na solicitação incluía nove fragmentos comuns da categoria geral `fragment`. Não se acrescentam identidades para compensar essa diferença. Os 45 craftings usam a prioridade da fonte: dust → gem → crystal → ingot → billet → ore, com o primeiro registro do snapshot conservador OreDict. Nenhum desses 45 cai em billet; caso um futuro material caia, o helper original `addBilletFragment` usa seis fragmentos, e a tabela conserva essa distinção. Um material sem saída é omitido com motivo, sem inventar `nothing`.

As 12 receitas da montadora usam os ingredientes NTM resolvidos, alternativas OreDict, tempo e energia originais. O arquivo principal passou de 400 para **412 receitas**. Não foram criadas receitas temporárias nem removidas pontes existentes. Os itens antigos `ore_bedrock__*` continuam no catálogo por compatibilidade, mas **não são usados pela nova cadeia**.

O fluido foi conferido em `content/fluids.json`: **id/string_id = `slop`**, source_variable `SLOP`.

## Depósitos no datapack

Há duas cadeias S6 de cinco arquivos (`structure_set → structure/jigsaw → template_pool/feature_pool_element → placed_feature → configured_feature`): `ntm:bedrock_overworld` e `ntm:bedrock_nether`. Tags de bioma restringem cada dimensão. A feature troca exclusivamente `minecraft:bedrock` por `ntm:ore_bedrock`; o tipo, grau, ácido e rendimento não são gravados no bloco.

| Parâmetro | Port |
|---|---|
| Piso Overworld | y = −64 |
| Piso Nether | y = 0; teto não é alvo |
| Frequência | rarity_filter chance 10, uma tentativa por chunk via structure_set spacing 1 |
| Forma | disk radius 1, half_height 0: cruz horizontal de cinco células, se todas forem bedrock |
| Identidade | Mesmo bloco nas duas dimensões; dados derivam do chunk no servidor |

**Desvio deliberado:** o Java tenta o centro e cada um dos oito vizinhos com 50%, formando até 3×3 (média de cinco blocos). O disco mantém a média, o limite horizontal 3×3 e a altura exata, mas tem forma fixa em cruz e não inclui os quatro cantos. Isso evita a dispersão vertical de uma feature de minério. A posição da tentativa usa a ponte S6 existente (in_square + offset 8), em vez do jitter legado de apenas duas posições por eixo; não se promete distribuição idêntica ao NTM. O revestimento de stone_depth de 7×7×6 do Java não é gerado. Não há alterações no motor ou em geradores de outras dimensões.

Arquivos e parâmetros estão em `tools/content-gen/bedrock_worldgen.py`; os dez JSONs gerados ficam em `content/data/ntm/worldgen/*/bedrock_{overworld,nether}.json`.

## Reprodução e verificação

```sh
/usr/local/bin/agent-run python3 tools/content-gen/generate.py
/usr/local/bin/agent-run python3 tools/visual-gen/generate.py
/usr/local/bin/agent-run env PYTHONHASHSEED=1 python3 -m unittest discover -s tools/content-gen -p 'test_*.py'
/usr/local/bin/agent-run env PYTHONHASHSEED=42 python3 -m unittest discover -s tools/content-gen -p 'test_*.py'
/usr/local/bin/agent-run python3 tools/content-gen/validate-bulk.py
/usr/local/bin/agent-run python3 tools/recipe-gen/validate.py
```

Os **56 testes Python** passaram em cada seed (oito novos para bedrock), além dos validadores de grafo e determinismo. Os testes cobrem metadados/ordem, slots duplicados, arredondamento/teto, omissão quando um fragmento é retirado, todos os IDs, parâmetros das brocas, crafting, identidade do fluido e datapack. A validação integral cobre 2.578 IDs, 861 receitas vanilla no datapack, 97 faixas/27 bits de radiação e oito abas criativas. Os dados de hazards foram regenerados e commitados pelo agente de máquinas, conforme o acordo de propriedade.

O build/check API 0.2 foi repetido após a correção do worldgen, numa cópia temporária de content: `/tmp/ntm-bedrock-content-fbr_iya6/target/horizon-mod/ntm.hmod`, **4.964.680 bytes**, SHA-256 `2fda6bae5b3b3384ac56a12c4fa19ad6cc58cafe8726ee44437e151f2bd8ecb6`. Este snapshot já inclui as entregas posteriores de bateria e cristalizador (2.580 IDs); substitui o pacote inicial anterior às correções. Nenhum pacote foi instalado no jogo real.

Evidências locais ficam em `content-gen-evidence/bedrock/`. `verify-bedrock-worldgen.py` cria um servidor dedicado temporário, silencioso e com seed fixa, pregera as duas dimensões e lê os MCA para conferir presença e altura dos depósitos. Execute sob `agent-run --user sacer`, com `xvfb-run`, `--horizon-client <binário>` e `--package <hmod temporário>`. HOME, opções e diretórios de dados ficam em `/tmp`; não é usado nenhum mundo do jogador. A aceitação funcional da escavadora/slopper pertence ao agente de máquinas.


### Medição concluída nos dois pisos

Em 30/10/2026, o servidor dedicado temporário completou a pregeneração com seed **1768123**. A leitura dos MCA de `/tmp/ntm-bedrock-worldgen-hklrd2b0` encontrou:

| Dimensão | Chunks examinados | Blocos `ntm:ore_bedrock` | Alturas observadas |
|---|---:|---:|---|
| Overworld | 81 | 56 | somente −64 |
| Nether | 81 | 40 | somente 0 |

Esses números contam **blocos**, não depósitos independentes. A amostra confirma presença e altura; não é uma estimativa precisa de frequência por chunk. As posições completas estão no JSON local `content-gen-evidence/bedrock/worldgen.json`, com logs de pregeneração e salvamento. Não houve erro ou conteúdo recusado no log. O processo terminou normalmente; HOME, opções, mundo e pacote ficaram em `/tmp`, sob Xvfb e sem áudio.

A primeira medição revelou zero blocos, apesar de as estruturas existirem: a âncora do jigsaw estava abaixo dos limites do mundo antes da execução da feature. O `start_height` da ponte S6 foi corrigido para 32, dentro do mundo, enquanto `height_range` da placed_feature continua fixando o piso −64/0. O provider do estado também usa o nome normalizado exigido pelo Horizon, `minecraft:simple`. Os testes verificam ambos os parâmetros. Essas correções não alteram o desvio de forma aprovado nem o contrato de rendimento por chunk.
