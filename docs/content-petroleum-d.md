# Petróleo, lote D: extração e depósitos

O lote acrescenta `machine_well`, `machine_pumpjack`, `machine_fracking_tower` e `machine_transformer`, todas no contrato beehive/facing/honey_level 0..5. Acrescenta também `ore_oil`, `ore_oil_empty`, `ore_bedrock_oil`, `ore_oil_sand` e `oil_pipe`. Os nomes en_us/pt_br e as texturas vêm do NTM; máquinas usam cubos opacos como fallback, e o visual-gen conserva seus arquivos.

O catálogo chega a **2.665 IDs**, sendo 401 blocos, 2.264 itens e 56 máquinas. Não foi necessário acrescentar fluido. Os dados de hazards e as abas foram regenerados. `oil_pipe` não aparece nas abas e não dropa item; os depósitos normais têm loot próprio. `ore_bedrock_oil` usa bedrock, hardness −1 e resistência 1.000.000; a areia usa o template sand, inclusive sua queda vanilla.

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

Passaram **118 testes Python em cada seed (1 e 42)**, `validate-bulk.py` e `tools/recipe-gen/validate.py`. Os testes incluem os quatro contratos de máquina, as taxas e probabilidades, o rendimento inesgotável do bedrock, a adaptação de altura, energia por tick, busca, construção e a regeneração em diretório novo. Logs em `content-gen-evidence/petroleum-d/validation.log`. Temporários grandes ficam em `.agent-tmp`, em disco.

A geração de depósitos e sua medição são uma unidade separada deste lote; o levantamento da API está em [Pesquisa de worldgen](content-petroleum-worldgen-research.md).
