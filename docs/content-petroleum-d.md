# Petróleo, lote D: extração e depósitos

O lote acrescenta `machine_well`, `machine_pumpjack`, `machine_fracking_tower` e `machine_transformer`, todas no contrato beehive/facing/honey_level 0..5. Acrescenta também `ore_oil`, `ore_oil_empty`, `ore_bedrock_oil`, `ore_oil_sand` e `oil_pipe`. Os nomes en_us/pt_br e as texturas vêm do NTM; máquinas usam cubos opacos como fallback, e o visual-gen conserva seus arquivos.

O catálogo chega a **2.665 IDs**, sendo 401 blocos, 2.264 itens e 56 máquinas. Não foi necessário acrescentar fluido. Os dados de hazards e as abas foram regenerados. `oil_pipe` não aparece nas abas e não dropa item; o óleo cheio dropa oil_tar__crude (Fortune, sem alternativa Silk Touch), como BlockOre; o vazio e a areia têm loot próprio. `ore_bedrock_oil` usa bedrock, hardness −1 e resistência 1.000.000; a areia usa o template sand, inclusive sua queda vanilla.

## Construção e receitas destravadas

As construções de poço, pumpjack e fracking tower resolveram na montadora. O transformador tem receita original de bancada; seu ID também destravou `ass.arcfurnace`. A montadora passa a **428 receitas**. A areia de óleo destravou uma receita na planta química (54 exportadas / 23 skipped), uma no liquefator (**32/2**) e uma no solidificador (**46/1**, além da entrada BALEFIRE substituída já documentada). O triturador passa a 730 receitas. O datapack soma 912 receitas vanilla.

## Extração algorítmica

`tools/content-gen/oil_extraction.py`, chamado pelo pipeline, exporta `content/machine-recipes/oil_extraction.json`, schema 1. O NTM não tem uma lista de receitas de poço: a extração depende da busca de blocos, do tanque e de probabilidade. Por isso `recipes` e `skipped` ficam vazios; `machine_config`, `rules` e as fontes registram a operação. Não foi inventada uma receita com o rendimento médio por bloco.

| Máquina | Buffer HE | HE por tick elegível | Intervalo de busca | Óleo por extração | Gás por extração | Chance de esgotar |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Poço | 100.000 | 100 | 50 ticks | 500 mB | 100–500 mB | 5% |
| Pumpjack | 250.000 | 200 | 25 ticks | 750 mB | 50–250 mB | 2,5% |
| Fracking tower | 5.000.000 | 5.000 | 20 ticks | 1.000 mB | 100–500 mB | 2% |

Cada tanque de óleo/gás comporta 64.000 mB. O fracker tem um terceiro tanque de fracksol da mesma capacidade e consome **10 mB por extração bem-sucedida**. Ao extrair do bedrock produz **100 mB de óleo e 10–50 mB de gás**, sem esgotar o depósito. O sorteio de gás é inteiro, uniforme e inclusivo; a produção é limitada pelo espaço do tanque.

O custo de energia ocorre em **cada tick elegível**, mesmo quando o tick não executa a busca periódica. A elegibilidade exige ambos os tanques de saída não cheios. A coluna vertical ignora oil_pipe, perfura o primeiro bloco com resistência a explosão menor que 1.000 e o substitui por oil_pipe. A busca percorre depósitos vazios em seis direções embaralhadas, com conjunto de visitados e profundidade máxima 64, até encontrar um depósito cheio. Cada valor e os métodos correspondentes conservam arquivo:linha.

Os dois extratores normais conservam a profundidade mínima 5 do NTM. O fracker adapta explicitamente **0 → −64** para alcançar o piso do Minecraft 26.3; ambos os valores ficam no JSON. Upgrades de energia/velocidade/overdrive, AFTERBURN, emissão de gases ao perfurar urânio/asbesto e OilSpot são comportamentos de lógica separados; a tabela não afirma implementá-los. O transformador entra como conteúdo, sem conversão de energia implementada nesta entrega.

## Verificação dos blocos e dados

Passaram **125 testes Python em cada seed (1 e 42)**, `validate-bulk.py` e `tools/recipe-gen/validate.py`. Os testes incluem os quatro contratos de máquina, as taxas e probabilidades, o rendimento inesgotável do bedrock, a adaptação de altura, energia por tick, busca, construção e a regeneração em diretório novo. Logs finais em `content-gen-evidence/petroleum-d/final-validation.log`. Temporários grandes ficam em `.agent-tmp`, em disco.

A geração de depósitos e sua medição são uma unidade separada deste lote; o levantamento da API está em [Pesquisa de worldgen](content-petroleum-worldgen-research.md).

## Medição de forma e volume

A referência geométrica vem de `MapGenBubble.java`: para cada raio inteiro uniforme 8..15, contam-se os pontos que satisfazem **x² + z² + 3y² < raio²/2**. Em pedra uniforme e sem recorte por cavernas, os volumes são 423, 639, 829, 1.105, 1.445, 1.855, 2.319 e 2.897, média **1.439 blocos**. É uma contagem exata da forma do código Java, não uma execução de worldgen no Minecraft 1.7.10. A geração natural substitui somente pedra e pode produzir menos.

A primeira rodada usou oito amostras por candidato na mesma inicialização, em um cubo de pedra 32³, refeito antes de cada chamada. A contagem e as extensões vieram dos blocos carregados pelo cliente após a operação e sua sincronização. Não houve conteúdo recusado. Evidências: `content-gen-evidence/petroleum-d/round1/`.

| Candidato | Volume médio | Mínimo–máximo | Extensões médias X × Y × Z |
| --- | ---: | ---: | --- |
| Ore, tamanho 64 | 680,375 | 640–718 | 14,125 × 8,25 × 14,875 |
| Geodo, filling 2 | 565 | 339–927 | 9,875 × 10,25 × 10 |
| Geodo, filling 3 | 972,875 | 690–1.768 | 11,875 × 12,125 × 11,875 |
| Geodo, filling 4 | 1.910,75 | 1.067–2.771 | 15,125 × 15,5 × 14,625 |

O ore de tamanho 64 conserva melhor o achatamento, mas rende cerca de metade do volume de referência. Os geodos têm bolsões mais arredondados. Por decisão do coordenador, uma única rodada adicional compara filling 3,5 e composições de dois/três ore64; o vencedor é o mais próximo de 1.439 blocos, com desempate pelo achatamento. O NTM usa o número de blocos como reserva de óleo, por isso o volume tem prioridade.

A prova explícita usa `tools/content-gen/build-oil-probe.py` e `verify-oil-features.py`. O Horizon base aceita as features do datapack na geração, mas não acrescenta seus IDs à lista usada para validar `/place feature`. A prova cria uma visão temporária dos recursos com symlinks e uma cópia própria de command-registries.json acrescida dos IDs de teste. Nenhum recurso real nem código do motor é alterado. A prova natural, em `verify-oil-worldgen.py`, usa os recursos originais e o datapack de produção, sem essa extensão de comandos.

A segunda e última rodada também usou oito amostras por candidato:

| Candidato | Volume médio | Mínimo–máximo | Diferença contra 1.439 | Extensões médias X × Y × Z |
| --- | ---: | ---: | ---: | --- |
| Geodo, filling 3,5 | 1.888,75 | 886–2.225 | +31,25% | 15,125 × 15,75 × 14,75 |
| Dois ore64 sobrepostos | 1.031,125 | 870–1.196 | −28,34% | 18,875 × 9 × 15,5 |
| **Três ore64 sobrepostos** | **1.244,375** | **1.153–1.351** | **−13,52%** | **17,625 × 9,75 × 19,75** |

Foi escolhida a composição de **três ore64**, a mais próxima do volume de referência e dentro da tolerância de ±15% definida pelo coordenador. O volume é a união dos blocos, não três vezes o volume de um veio: as partes se sobrepõem. A forma continua irregular, mas o achatamento médio é mais próximo do elipsoide do NTM. O geodo 3,5 teve seis amostras grandes e duas pequenas; a amostra é limitada e não é uma estimativa precisa de toda a distribuição.

O 26.3 não tem o tipo antigo random_patch. A composição usa `minecraft:random_selector` com fallback colocado, `count:3` e offset horizontal uniforme de −2 a +2; Y não recebe jitter adicional. Cada veio tem size 64, limite suportado pelo motor, e só substitui `minecraft:stone`. O perfil usa origins em Y 15..39; o deslocamento vertical interno do ore vanilla e a forma dos três lóbulos permanecem diferenças em relação ao MapGenBubble. Não há crustas nem preenchimento sobre água, cavernas ou outros minérios. Os tags de proteção dos geodos ficam apenas nos pacotes comparativos.

A mesma inicialização da segunda rodada conferiu `/setblock` e `/give` para os **nove IDs do D**, incluindo os quatro estados de máquina facing=north/honey_level=5 e os cinco blocos comuns. Todos voltaram com o ID esperado e quantidade 1 no inventário; a captura de fallback é `content-gen-evidence/petroleum-d/round2-blocks.png`. O pacote comparativo era SHA-256 `52317afe5002d092168876c9705d4bec47e58ca23474757e4970367b37824f04`; não houve conteúdo recusado. Isso verifica conteúdo e estados, sem afirmar lógica funcional nas máquinas Inert.

## Perfis de geração

O pipeline emite cinco pontes S6 (structure_set → jigsaw → template_pool → placed_feature → configured_feature), como o worldgen já medido do bedrock. Os salts são independentes. A inclusão no pipeline não depende de uma API nativa nem altera o motor.

| Perfil | Substrato | Distribuição |
| --- | --- | --- |
| oil_bubble | pedra | três ore64, 1/100 por chunk, origins Y 15..39 |
| oil_bubble_hot_dry | pedra, bioma quente e seco | tentativa adicional 1/49 |
| oil_bedrock | bedrock, pedra ou deepslate | disco raio 4, cinco camadas −64..−60, 1/200 |
| oil_sand | areia normal ou vermelha, bioma quente e seco | disco raio 8, nove camadas, origins Y 60..67, 1/200 |
| oil_sand_hot_dry | os mesmos | tentativa adicional 1/98 |

O tag de bioma inclui desert, badlands, wooded_badlands e eroded_badlands, correspondendo aos biomas vanilla atuais de temperatura ≥2 e chuva nula. A probabilidade combinada de ao menos uma tentativa é **3,0204%** para óleo, contra 1/33 = 3,0303% no NTM, e **1,5153%** para areia de óleo, contra 1/66 = 1,5152%. As tentativas são independentes: raramente um chunk recebe as duas. O número esperado de tentativas difere ligeiramente da probabilidade de pelo menos uma; a frequência não é uma alegação de geração idêntica à do Java.

O depósito de bedrock original tem **205 pontos** em substrato uniforme (|x|<5, |z|<5, Y 0..4 e |x|+Y+|z|≤6). O disco escolhido tem 245 pontos ideais, +19,51%, e perde o afunilamento. Ele substitui deepslate para adaptar o substrato atual às camadas negativas. A areia usa um disco menor que as bolhas fuzzy de raio 16..47 do NTM; mantém os substratos e a faixa Y 56..71, mas não o volume/contorno exatos. A degradação da superfície, o buraco/vazamento, as manchas e as plantas continuam lacunas explícitas, como autorizado.

## Medição natural concluída

O servidor temporário, mudo, gerou e salvou **914 chunks** no seed 1768123, usando os recursos originais do Horizon e o pacote de produção SHA-256 `06a13393e0c34fb170fb5c14004ee0e3730a651eceadaaf4074746715df9b099` (5.620.208 bytes). O script terminou com código 0, sem conteúdo recusado. O scan leu os blocos do MCA após save-all flush e encerramento do servidor.

| Região | Chunks | Óleo | Óleo de bedrock | Areia de óleo |
| --- | ---: | ---: | ---: | ---: |
| Centro 0,0, raio 192 | 625 | 3.268 blocos | 730 blocos | 0 |
| Deserto localizado em 416,−1728, raio 128 | 289 | 2.972 blocos | 590 blocos | 0 |

Todos os **1.320 blocos de óleo de bedrock** estão nas cinco camadas −64..−60, com a mesma contagem por camada em cada região. Os depósitos completos têm 245 blocos; os componentes cortados pelo limite do scan têm menos. O óleo comum apareceu entre Y 15 e 41 nesta amostra; Y 40..41 decorre da extensão vertical interna do ore vanilla além do origin, diferença já declarada. Os componentes naturais não equivalem a bolsões completos: cavernas, pedra ausente, limite do scan e fragmentação quebram a união em componentes menores. Por isso a média de volume escolhida vem da prova controlada em pedra uniforme, não da média desses componentes.

**Não apareceu areia de óleo nas duas regiões.** Isso limita a cobertura da aceitação: a presença natural e o volume desse perfil não foram demonstrados. Os testes estáticos conferem o substrato, o filtro e a faixa nominal; a medição não prova a frequência real nem substitui uma prova positiva desse perfil. O relatório preserva esse zero sem atribuí-lo a uma causa não verificada.

Evidências: `content-gen-evidence/petroleum-d/worldgen.json`, `worldgen-control.json` e `worldgen-server.log`. O HEAD isolado `1f54a10` passou **259 testes de ntm-core**, e hazard-gen não alterou hazards/radiation_buckets. Os agentes de lógica e visual também conferiram o HEAD compartilhado posterior com workspace e clippy antes do push conjunto.
