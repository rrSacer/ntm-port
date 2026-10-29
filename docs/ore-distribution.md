# Distribuição dos minérios: NTM 1.7.10 → Horizon 26.3

Extração local em 2026-10-28 do NTM `4f5924e2b9e1e49cee61787674d2658acb3d559b`. Os números abaixo são os **defaults efetivos de `loadFromConfig`**, não de uma configuração particular do jogador. São tentativas de veio, não número garantido de blocos.

Fontes: [WorldConfig.java](/home/sacer/Workspace/ntm-src/src/main/java/com/hbm/config/WorldConfig.java:120), [HbmWorldGen.java](/home/sacer/Workspace/ntm-src/src/main/java/com/hbm/lib/HbmWorldGen.java:98), [DungeonToolbox.java](/home/sacer/Workspace/ntm-src/src/main/java/com/hbm/world/generator/DungeonToolbox.java:65). `y = minHeight + nextInt(variance)`: o limite superior é exclusivo na chamada e inclusivo nas faixas desta tabela. O tamanho é o parâmetro `WorldGenMinable`, não o volume final; o veio pode ultrapassar a faixa das origens.

| Material | Bloco NTM | Tentativas/chunk | Tamanho | Y da origem, inclusivo | Dimensão / alvo |
| --- | --- | ---: | ---: | --- | --- |
| Urânio | `ore_uranium` | 7 | 5 | 5–24 | Overworld / stone |
| Tório | `ore_thorium` | 7 | 5 | 5–29 | Overworld / stone |
| Titânio | `ore_titanium` | 8 | 6 | 5–34 | Overworld / stone |
| Chumbo | `ore_lead` | 6 | 9 | 5–34 | Overworld / stone |
| Cobre | `ore_copper` | 12 | 6 | 5–49 | Overworld / stone |
| Cobre vermelho | — | — | — | — | Liga produzida a partir de cobre + redstone; sem minério natural |
| Alumínio | `ore_aluminium` | 7 | 6 | 5–44 | Overworld / stone |
| Tungstênio | `ore_tungsten` | 10 | 8 | 5–34 | Overworld / stone |
| Berílio | `ore_beryllium` | 6 | 4 | 5–34 | Overworld / stone |
| Lítio | `ore_gneiss_lithium` | 6 | 6 | 30–39 | Overworld / stone_gneiss |
| Amianto | `ore_asbestos` | 2 | 4 | 16–31 | Overworld / stone |
| Fluorita | `ore_fluorite` | 6 | 4 | 5–49 | Overworld / stone |
| Salitre | `ore_niter` | 6 | 6 | 5–34 | Overworld / stone |
| Enxofre | `ore_sulfur` | 5 | 8 | 5–34 | Overworld / stone |
| Urânio em gnaisse | `ore_gneiss_uranium` | 21 (= 7×3) | 6 | 30–39 | Overworld / stone_gneiss |
| Cobre em gnaisse | `ore_gneiss_copper` | 36 (= 12×3) | 6 | 30–39 | Overworld / stone_gneiss |
| Amianto em gnaisse | `ore_gneiss_asbestos` | 6 (= 2×3) | 6 | 30–39 | Overworld / stone_gneiss |
| Urânio do Nether | `ore_nether_uranium` | 8 | 6 | 0–126 | Nether / netherrack |
| Tungstênio do Nether | `ore_nether_tungsten` | 10 | 10 | 0–126 | Nether / netherrack |
| Enxofre do Nether | `ore_nether_sulfur` | 26 | 12 | 0–126 | Nether / netherrack |

`HbmWorldGen.generate` seleciona Overworld em dimensão 0, Nether em -1 e End em 1. Nas demais, chama geração de superfície quando `enableMDOres=true` (default). As tabelas são sujeitas aos switches `overworldOre`/`netherOre` e, no caso do gnaisse, à disponibilidade do substrato. Nenhum dos materiais pedidos tem chamada equivalente em `generateEnd`. Depósitos de profundidade, clusters, cavernas de enxofre/amianto e outros geradores especiais são sistemas adicionais; os números acima não somam sua produção.

Duas armadilhas na extração: o inicializador de `uraniumSpawn` diz 6, mas a configuração inicializa com **7**; `asbestosSpawn` começa em 4, mas a configuração inicializa com **2**. Lítio tem parâmetro, porém a chamada localizada é só a variante de gnaisse. Cobre vermelho é a liga `ingot_red_copper`, confirmada em [BlastFurnaceRecipesNT.java](/home/sacer/Workspace/ntm-src/src/main/java/com/hbm/inventory/recipes/BlastFurnaceRecipesNT.java:46); não criar `ore_red_copper` como se fosse original.

## Tradução proposta

A cadeia comprovada no [spike S6](../spikes/s6/REPORT.md) é `structure_set → jigsaw size=1 → feature_pool_element rigid → placed_feature → ore`. Não requer datapack de funções nem mudança no motor: JSON de worldgen faz parte do content mod API 0.2. Esta tabela é uma proposta de balanceamento; **o spike verificou duas features tamanho 8, spacing 2 e uma tentativa por início**, não os 13 materiais com suas densidades finais.

Para preservar aproximadamente as tentativas por chunk de cada linha:

1. Um `structure_set` **separado por material/faixa/dimensão**, `random_spread`, `spacing:1`, `separation:0`, salt próprio e estável. Um único set com várias estruturas escolhe entre elas, não gera todas. O spacing 2 do spike dá um início por quatro chunks: copiar os números da tabela como `count` com spacing 2 reduz a densidade média a 25%.
2. Estrutura `minecraft:jigsaw`, `size:1`, `terrain_adaptation:none`, `start_pool` próprio e `biomes` restrito à dimensão; pool com `feature_pool_element`, `projection:rigid`. `size:0` remove as peças neste motor. Sem `minecraft:biome` no placement: esse caminho não tem a feature vinculada à lista de features de um bioma vanilla.
3. Placement proposto: `count:N`, `in_square`, `offset {x:8,y:0,z:8}`, `height_range uniform min/max absolutos da tabela`. `in_square` acrescenta 0–15 em x/z e o offset conserva a origem deslocada para a área de decoração. Só o offset constante foi testado em S6; validar densidade e escritas nas bordas antes de adotar a composição com count/in_square.
4. `configured_feature` com `type:minecraft:ore`, `size` da tabela e `targets` **na raiz**, sem `config`. Cada alvo usa `block_match` e `state:{"id":"ntm:ore_…"}`. Tamanho 1.7.10 e tamanho 26.3 não garantem o mesmo número de blocos: medir rendimento por chunk antes de ajustar.
5. Overworld: manter as alturas originais dá prioridade à stone. Acrescentar alvo deepslate com variante própria permite atravessar a transição, mas não cria distribuição na nova profundidade negativa. Uma segunda faixa negativa é uma **decisão nova de balanceamento**, não extração do NTM. No spike, ela foi -54…-8 só para provar a variante.
6. Gnaisse e lítio: preservar gnaisse exige gerar primeiro esse substrato e a variante correspondente. Se o M1 não gerar gnaisse, lítio em stone/deepslate com N=6, size=6 e Y=30…39 é uma adaptação explícita a aprovar no recorte, não equivalência do original. Nether usa seus biomas e alvo netherrack, com os números próprios da tabela; o Nether não foi testado no S6.

Exemplo proposto para urânio normal (N=7, size=5, Y=5…24):

```json
{
  "feature": "ntm:uranium",
  "placement": [
    {"type":"minecraft:count","count":7},
    {"type":"minecraft:in_square"},
    {"type":"minecraft:offset","x":8,"y":0,"z":8},
    {"type":"minecraft:height_range","height":{"type":"minecraft:uniform","min_inclusive":{"absolute":5},"max_inclusive":{"absolute":24}}}
  ]
}
```

A geração pelo sistema de estruturas ocorre antes de parte da decoração vanilla. O controle sem mod mostrou que algumas posições onde há minério do mod acabam recebendo gravel/granite/ores vanilla sem o mod. Isso não prova substituição desses blocos pelo mod: o teste controlado do S6 confirma o filtro por material. Significa que a coexistência e o rendimento dos minérios vanilla podem mudar; medir na calibração do M1.

## Implementação M1 e verificação no dedicado (2026-10-28)

O gerador real em `tools/content-gen/generate.py`, dirigido por `m1.json.worldgen`, agora escreve 55 arquivos: onze cadeias de cinco registros no namespace `ntm`. Mantém os defaults efetivos da tabela, `spacing:1`, `separation:0`, `count:N`, `in_square`, offset 8/0/8 e salts distintos entre 2167000 e 2167170. Os alvos são matches exatos de stone/deepslate; não existe `minecraft:biome` no placement. Não se acrescentou uma faixa negativa de altura. Ambos os substratos usam o ID e a arte normal do minério NTM; variantes visuais de deepslate ficam para depois.

Lítio permanece uma lacuna de geração: seu bloco de gnaisse está registrado, mas o M1 não gera o substrato `stone_gneiss`. Cobre vermelho continua sendo liga. As quantidades abaixo não incluem gnaisse, Nether, clusters, cavidades especiais nem sistemas adicionais do NTM.

Seed **1768123**, dois mundos dedicados independentes, cada um com **289 chunks pregerados** e comparação dos **81 chunks centrais** (x/z -64…79). O halo evita confundir chunks vizinhos ainda em geração com diferenças de densidade. A primeira amostra deu:

| Material | Tentativas NTM/chunk | Tamanho | Y da origem | Blocos em 81 chunks | Blocos/chunk | Blocos por tentativa declarada |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| Urânio | 7 | 5 | 5–24 | 1113 | 13,74 | 1,96 |
| Tório | 7 | 5 | 5–29 | 1032 | 12,74 | 1,82 |
| Titânio | 8 | 6 | 5–34 | 1553 | 19,17 | 2,40 |
| Chumbo | 6 | 9 | 5–34 | 1908 | 23,56 | 3,93 |
| Alumínio | 7 | 6 | 5–44 | 1509 | 18,63 | 2,66 |
| Tungstênio | 10 | 8 | 5–34 | 2763 | 34,11 | 3,41 |
| Berílio | 6 | 4 | 5–34 | 575 | 7,10 | 1,18 |
| Amianto | 2 | 4 | 16–31 | 156 | 1,93 | 0,96 |
| Fluorita | 6 | 4 | 5–49 | 625 | 7,72 | 1,29 |
| Salitre | 6 | 6 | 5–34 | 1167 | 14,41 | 2,40 |
| Enxofre | 5 | 8 | 5–34 | 1477 | 18,23 | 3,65 |

A densidade traduzida está correta quanto a **tentativas/tamanho/altura**: todos os onze materiais aparecem, com cerca de 0,96–3,93 blocos por tentativa declarada. Isso é um rendimento plausível para veios cujo `size` é 4–9 e que nem sempre encontram substrato livre. Não houve oracle Java 1.7.10 para provar igualdade exata do número de blocos; `WorldGenMinable` e o minério 26.3 têm geometrias diferentes. A segunda amostra muda apenas o tório para 1031, o berílio para 576 e o alumínio para 1510; os demais valores se repetem.

**Limitação aceita para o M1: seed igual não garante igualdade estrita em sobreposições.** As duas amostras têm 13.878 e 13.879 blocos. Duas coordenadas diferem:

| Coordenada | Mundo A | Mundo B |
| --- | --- | --- |
| `-54 12 -9` | `ntm:ore_thorium` | `ntm:ore_beryllium` |
| `-47 5 -13` | `minecraft:granite` | `ntm:ore_aluminium` |

A causa provável é a ordem de aplicação de features que disputam a mesma posição: duas cadeias do mod no primeiro ponto, minério e decoração vanilla no segundo. Os alvos exatos não autorizam substituir granite: uma ordem diferente pode criar granite antes da tentativa do minério ou preservar o minério já criado. O controle de substratos é separado para não confundir os estados finais com esse filtro.

O ensaio inicial sem halo teve 44 diferenças, concentradas nas bordas e em chunks vizinhos. Com halo restam somente as duas sobreposições acima. O teste estrito em `verify-worldgen.py` continua retornando falha (`same_seed_same_blocks:false`), para não ocultar a limitação. O coordenador aceitou expressamente esse resultado para o M1 e orientou não investigar agora a alternativa **cadeia overlay única com ordem fixa**. Essa alternativa fica como melhoria futura opcional e ainda estaria sujeita à competição com features vanilla.

Evidência e reprodução: `content-gen-evidence/worldgen/generation-results.json`, `stable-initial-scan.json`, `stable-repeat-scan.json`, logs e caminhos dos saves temporários. Execute `python3 tools/content-gen/verify-worldgen.py`; a porta dedicada é 25731. Não usar `/place feature ntm:*` como teste: o registro de argumentos desse comando não contém as features carregadas pela ponte.

### Controle final de substrato

Os mesmos onze configs foram executados em quatro mundos diagnósticos separados, com cubos conhecidos de stone/deepslate e controles dirt/andesite/bedrock/air. São 81 chunks centrais examinados por mundo (**324 chunks** no total). **Todos os onze minérios apareceram em stone e em deepslate; zero minério apareceu nos quatro materiais incompatíveis ou fora das faixas dos cubos.** O JSON de resultados está em `content-gen-evidence/worldgen/target-checks.json` (`pass:true`).

Dois desenhos anteriores de diagnóstico ficaram preservados como evidência de limitações do teste: aplicar todos os minérios na mesma posição deixava alguns sem amostra positiva por competição; colocar cada um em sua própria altura, acima do terreno natural, fazia o minério ser descartado pelo teste `OceanFloorWg`, mesmo após preencher cubos de pedra. O teste final usa lotes de até três minérios, cubos separados e controles positivos abaixo da superfície. Não foi necessário alterar os configs reais nem o motor.
