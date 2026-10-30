# Energia, lote 1: conteúdo dos aquecedores

O catálogo ganha **quatro máquinas**: `ntm:heater_oven`, `ntm:heater_oilburner`, `ntm:heater_electric` e `ntm:heater_heatex`. Todas usam beehive, facing e honey_level 0..5, nomes en_us/pt_br do NTM, loot próprio e texturas opacas de fallback. O total passa a **2.679 IDs: 413 blocos, 2.266 itens e 62 máquinas**. Modelos, telas e multiblocos pertencem ao visual-gen; o conteúdo não declara essa geometria fiel como implementada.

As construções originais foram destravadas em `machine-recipes/anvil.json`: oven/oilburner exigem bigorna tier 2; electric/heatex, tier 3. Também entrou a desmontagem original do oven. Assim a seção de construção passa de **105/152 para 110/147** (emitidas/skipped), sem receita temporária de bancada. Nenhum ingrediente novo foi necessário neste lote; os componentes que faltam para pistões, Stirling e RTG continuam associados aos próximos lotes que os usam.

## Dados térmicos separados dos IDs

`content/fluid_traits.json`, schema 1, conserva **60 FT_Flammable finais** e **16 FT_Coolable**, em ordem de declaração, com fonte por linha. Flammable guarda TU/balde, incluindo as atualizações e os arredondamentos de registerCalculatedFuel; a origem aponta para a atualização final. Coolable guarda fluido, amountReq, saída/amountProduced, heatEnergy e eficiências TURBINE/HEATEXCHANGER. A ausência de eficiência fica explicitamente em zero, como getEfficiency no Java.

Todos esses fluidos já existem nos 157 tipos do core. Nenhum tipo ou fluido novo foi inventado. FT_Combustible e FuelGrade entram no lote 3; o máximo entre calor e combustão usado pelo coqueificador não serve como energia de um motor. O novo arquivo não altera fluids.json.

`content/machine-recipes/heaters.json` é um contrato algorítmico, com **85 combustíveis de oven** e **16 operações de heatex**, além de machine_config por ID. `recipes` e `skipped` ficam vazios; os dados não são falsamente expostos como receitas generic com duração ou HE/t. Os campos que exigem implementação ficam em requires_runtime_fields.

| Aquecedor | Regras do NTM conservadas |
|---|---|
| Oven | ModuleBurnTime igual ao firebox, calor-base 500, duração ×0,125, teto 500.000 TU. Recebe calor abaixo com eficiência 0,5, consumindo a quantidade integral na fonte. Há crédito de calor na ignição; a contagem de queima pausa ao atingir o teto. |
| Oilburner | Tanque 16.000 mB, rate inicial 1 e máximo 10 mB/t sem upgrade, teto 500.000 TU. A divisão inteira TU/balde÷1000 ocorre **antes** de multiplicar pelo combustível consumido. Pode ultrapassar o teto no tick de queima e é limitado no início do seguinte. O ramo válido de combustível suprime resfriamento mesmo com tanque vazio. |
| Electric | Setting 0..10, consumo floor(setting^1,4×200) HE/t e geração setting×100 TU/t; buffer de HE equivale ao consumo atual×20. Primeiro perde 0,1% do calor e recebe o calor de baixo a 85%. Não há teto explícito de TU na fonte; isOn é derivado do funcionamento. |
| Heatex | Dois tanques de 24.000 mB; primeiro perde 0,1% do calor, depois converte FT_Coolable com HEATEXCHANGER positivo. amountToCool limita **operações**, não mB. O intervalo depende do worldTime. Exemplo: 100 mB steam→1 mB spentsteam, 100 TU úteis por operação. Não usa heating_steps de FT_Heatable. |

As constantes, fórmulas, ordem de operações e fontes estão no JSON. O agente de lógica registrou os quatro como **Inert** em `0058f5b`; sua implementação funcional é uma frente separada. Este lote aceita conteúdo, contratos e construções, sem afirmar produção efetiva de calor.

## Lacunas explícitas

O snapshot de 85 combustíveis reutiliza FuelHandler, o perfil vanilla legado e o OreDict conservador da fundição. Há **três entradas de lacuna** herdadas: book_guide ausente, coal_eternal ausente e a exclusão geral de combustíveis externos/configuráveis, vanilla moderno e subclasses genéricas de madeira não verificadas. Esses dois itens opcionais não bloqueiam as quatro construções nem a alimentação com carvão. Não se aplica aqui o filtro de combustível restrito do alto-forno.

Ashpit/cinzas, fuligem/poluição, liberação de fluidos, som e upgrades OVERDRIVE precisam de suporte de lógica. Os modelos automáticos que o visual-gen chegou a extrair não fazem parte da aceitação de fidelidade deste lote; o visual pode removê-los em favor do fallback. O manifesto de propriedade do visual foi respeitado.

## Verificação

- **144 testes Python por seed (1 e 42)**; oito testes novos verificam IDs/estados, construções na bigorna, combustível/recipiente, valores e fontes finais de flammable, proporções/eficiências de coolable, ausência de power/duration inventados, fluidos conhecidos e regeneração em diretório novo.
- `validate-bulk.py` e `tools/recipe-gen/validate.py` passaram; a validação vanilla agora confere também IDs usados como chaves de tabelas de combustível. Hazards, abas e cobertura foram regenerados.
- O agente de lógica conferiu a exportação isolada com workspace e clippy sem warnings após registrar os quatro Inert.
- Build/check passou: **6.257.309 bytes**, SHA-256 `ff44fc4f09f6cab790aa94b87b516072c9a2acea3b412ba29b5f79adafd34ec7`.
- A prova obrigatória usou horizon-patched, Xvfb, perfil temporário em disco, HORIZON_NO_AUDIO, options em zero e mute.py: **57 conferências**, /setblock dos quatro IDs com facing=north/honey_level=0 e 5, e /give dos quatro IDs conferidos no inventário. O detector não encontrou rejeição do mod.

Evidências locais em `content-gen-evidence/energy-heaters/`: build.log, game.json, game-checks.json, game-client.log, machines.png e items.png. As regressões completas estão em `content-gen-evidence/energy-heaters-validation.log`; geração em energy-heaters-generation.log. O pacote de prova incluiu a sobreposição visual corrente; a prova valida carregamento, IDs/estados e itens, não a fidelidade desses modelos ou lógica funcional. Nenhum worldgen mudou neste lote. Os temporários grandes são removidos após preservar as evidências.

A exportação isolada do HEAD conjunto `1f3a7d8` também passou `cargo test --workspace` (incluindo **276 testes do ntm-core**) e `cargo clippy --workspace -- -D warnings`; a reextração de hazards/radiation_buckets não alterou os dados. Esse HEAD já inclui os commits prontos de visual/portas dos lotes E/D que sobem no push conjunto.
