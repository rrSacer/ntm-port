# Petróleo, lote E: auxiliares e lacunas do catálogo

Este lote acrescenta **dez IDs**: `machine_hephaestus`, `machine_flare`, `bio_wafer`, `pill_herbal` e os seis `plant_flower__<enum>` (`foxglove`, `tobacco`, `nightshade`, `weed`, `cd0`, `cd1`). O catálogo chega a **2.675 IDs**, 409 blocos, 2.266 itens e 58 máquinas. Não há fluido novo nem perfil de worldgen novo neste lote.

As máquinas seguem beehive/facing/honey_level 0..5, têm nomes en_us/pt_br, loot e cubos opacos de fallback. O visual-gen conserva seus modelos, telas e multiblocos. As seis plantas mantêm a metadata e o enum de origem, usam template poppy com modelo cruzado e as texturas originais `plant_flower.<enum>`. CD1 dropa CD0 como no NTM. Os dois consumíveis entram como componentes, com ícones e nomes originais; não recebem efeitos de comida ou medicamento inventados.

## Receitas resolvidas

| Tabela | Antes do E | Depois do E | O que resolveu |
| --- | --- | --- | --- |
| Montadora | 428 exportadas / 203 skipped | **430 / 201** | ass.hephaestus e ass.flarestack |
| Liquefação | 32 / 2 | **34 / 0** | duas metadata plant_flower |
| Solidificação | 46 / 1 | **47 / 0** | bio_wafer; a BALEFIRE substituída continua separada |
| Misturador | 49 / 1 | **50 / 0** | pill_herbal, alternativa de PHEROMONE_M |

As receitas vanilla de construção/itens também foram regeneradas; o datapack passa a **918 receitas**. O gerador conserva os motivos das outras lacunas da montadora. Não houve receita temporária para substituir máquina ausente. O fallback dinâmico de comida→salient da liquefação permanece uma lacuna de runtime já documentada no lote A; zerar skipped não implica resolver esse comportamento dinâmico.

## Hephaestus

`content/machine-recipes/hephaestus.json` exporta **nove conversões**, sem skipped, a partir dos 18 passos de FT_Heatable em Fluids.java. Entra apenas o **primeiro passo** dos fluidos com HEATEXCHANGER>0: água, quatro óleos, coolant, perfluoromethyl, mug e sangue. Input/outputFluids e a quantidade vêm do passo; `heat_required_tu` conserva o heatReq bruto. A água exige 200 TU para 1 mB → 100 mB de steam. A eficiência 0,25 só controla a elegibilidade; o tile não divide heatReq por ela.

O calor é uma **oferta por tick**, calculada a partir dos dez planos armazenados, e não um buffer TU consumido. Em cada tick a máquina atualiza um plano 15×15 abaixo dela, em height=worldTime%10 e y=machineY−1−height. Lava contribui 5 por bloco, volcanic_lava_block 150 e ore_volcano 300. Encontrar ore_volcano atualiza fissureScanTime; o total é triplicado quando worldTime−fissureScanTime<20. A operação converte o mínimo de inputFill/amountReq, espaço de saída/amountProduced e totalHeat/heatReq, com divisões inteiras. Ambos os tanques têm 24.000 mB.

O JSON registra arquivo:linha para tanques, scan, elegibilidade, conversão e cálculo do calor. Não foi inventada duração nem consumo de HE. `power:0` e os campos adicionais exigem adaptação explícita do runtime, como combinado com o agente de lógica. O registro inicial é Inert.

Lacunas: o scan original exige y>=0; a tabela retém essa regra e **não presume uma adaptação para −64**. volcanic_lava_block e ore_volcano não estão no catálogo, então suas contribuições permanecem documentais. Modelos, animação, sons e a implementação térmica são trabalhos separados.

## Flare

`content/machine-recipes/flare.json` conserva os atributos de **156 fluidos**, em ordem de declaração, exceto NONE. Há **60 fluidos com FT_Flammable** e **38 ventáveis** (GASEOUS ou GASEOUS_ART/EVAP). A energia usa o calor final de FT_Flammable, inclusive os registerCalculatedFuel e os arredondamentos do Java; não usa FT_Combustible como substituto.

A flare tem tanque de 64.000 mB, buffer de 100.000 HE, ventila até 50 mB/tick ou queima até 10 mB/tick sem upgrades. SPEED multiplica os limites por 1+nível. O modo on e o modo burn começam desligados. Sem burn, ou sem FT_Flammable, só um fluido gasoso pode ser ventilado. Com burn e FT_Flammable, o fluido é consumido mesmo com o buffer de energia cheio.

A fórmula inteira é preservada por etapas: `base=(calorPorBalde×mB)//1000`, depois `base//=penalidade`, e `produção=base+(base×EFFECT)//3`. A penalidade é 5 para GASEOUS/GASEOUS_ART e 10 para os demais. A energia acumulada é limitada a 100.000. O JSON é um contrato algorítmico (`fuel_traits` e `machine_config`), sem uma falsa receita consumível generic. Poluição, tilt, traits de liberação, fogo/dano, partículas e som são lacunas de lógica; nenhum efeito é alegado nesta entrega.

## Consumíveis e plantas

`bio_wafer` e `pill_herbal` usam template neutro iron_ingot. Nutrição, consumo e os efeitos médicos/colaterais de ItemPill estão marcados em behavior_gap. As plantas usam a sobrevivência vanilla da poppy; crescimento, necessidade de solo oleoso, regeneração CD1→CD0 e interações do NTM precisam de lógica. As plantas não foram acrescentadas à degradação de superfície do petróleo: essa lacuna continua explícita no lote D.

## Verificação

Passaram **132 testes Python por seed (1 e 42)**, `validate-bulk.py` e `tools/recipe-gen/validate.py`. Há sete testes específicos do E: os contratos/construções das máquinas, os seis enums/texturas/drop, a neutralidade dos consumíveis, elegibilidade/heatReq do Hephaestus, oferta de calor, os 156 traits/fórmula da flare e a regeneração em diretório novo. Hazards e abas foram regenerados. Evidências em `content-gen-evidence/petroleum-e/`.

Build/check passou no pacote de **6.025.392 bytes**, SHA-256 `5d6e14757de67663f8938294f8ab1e225657af3976e8742caf7919b6a8d2b08b`. O agente de lógica registrou as duas máquinas como Inert em `954a619` e conferiu o HEAD isolado com `cargo test --workspace` e clippy sem warnings. A amostra de conteúdo em cliente temporário mudo está enfileirada; seus resultados são anexados quando voltar. A verificação de conteúdo e estados não equivale a aceitar a lógica funcional das duas máquinas.


## Correção da rejeição do pacote em 26.3

A primeira prova em cliente encontrou uma regressão que build/check e os validadores anteriores não cobriam: as seis plantas tinham `properties.sounds: minecraft:grass`. Esse campo referencia um **bloco do registro atual** para copiar os sons, e não um evento de som. O bloco legado não existe em 26.3; por isso o motor rejeitava o mod de conteúdo inteiro, com `content mods not loaded`. O sucesso anterior de build/check não demonstrava carregamento no jogo.

A fonte das plantas agora usa `minecraft:short_grass`. O importador também normaliza explicitamente esse nome legado; outras referências continuam sujeitas à validação, sem substituições adivinhadas. A auditoria de todos os sons emitidos encontrou apenas essas seis referências inválidas: stone, iron_block, anvil e sand já existiam no registro. O bloco interno sem override de som herda seu template.

`tools/content-gen/vanilla_references.py` confere templates em registros separados de blocos e itens, blocos usados como sons, loot, receitas, tags, ícones, entradas/saídas de máquinas e states/predicados do worldgen. Ele consulta `registries.json` da exportação 26.3; `HORIZON_RESOURCES` permite escolher sua pasta. O gerador completo e `validate-bulk.py` chamam a checagem. Recursos de modelo, textura e tipos de feature não são confundidos com IDs de itens/blocos. Os testes novos reproduzem a rejeição de grass e referências ausentes em cada contexto, inclusive um item válido usado indevidamente como bloco.

Passaram **136 testes Python em cada seed (1 e 42)** e os dois validadores. As provas detectam explicitamente `content mods not loaded` e abortam durante a inicialização. A repetição usa o executável patched, com perfil temporário em disco, Xvfb e áudio desativado. A prova positiva confirmou 75 comandos/conferências: Hephaestus e flare com facing=north nos níveis 0 e 5, os seis blocos de planta sobre solo e oito itens por /give. O log não contém rejeições. Evidências: game.json, game-checks.json, game-client.log, machines.png e items.png em content-gen-evidence/petroleum-e/. O pacote usado tem 6.071.431 bytes e SHA-256 `3ebb2068b266236cd06c22f38db0784c85617500b52887437540f5e6d0ede321`. Esta prova confirma carregamento e IDs/estados; não aceita a lógica funcional das duas máquinas.

A exportação isolada do HEAD `04e6066` passou **270 testes de ntm-core**; a reextração de hazards e radiation_buckets não alterou os dados. Nenhuma mudança de lógica Rust fez parte desta correção.
