# Energia, lote 5: RTG e radgen

O lote acrescenta 70 identidades originais: `ntm:machine_rtg_grey` (variável Java `machine_rtg`), `ntm:machine_radgen`, dez pellets RTG vivos, seis pellets esgotados e 52 resíduos do radgen (oito classes curtas e cinco longas, cada uma em normal/tiny/vivo/esgotado). Os dois blocos usam beehive com facing horizontal e honey_level 0..5 e ficam Inert; visual-gen mantém `auto: false`. Texturas e nomes vêm do fonte; os nomes dos resíduos incluem a classe isotópica que o original exibia no tooltip.

O catálogo chega a 2.771 IDs (429 blocos e 2.342 itens). As dez receitas originais dos pellets vivos foram destravadas, totalizando 948 receitas vanilla. A montadora ganha as construções de RTG e radgen, chegando a 443 operações. A tabela Rust correspondente foi regenerada pelo machine-gen, com aprovação da frente de lógica.

`machine-recipes/radioisotopes.json` conserva os contratos algorítmicos:

- RTG: 15 slots, buffer de 100.000 HE, saída antes da geração do tick, calor limitado a 600 TU com decaimento ou 200 sem ele e conversão de 5 HE/TU. Os dez pellets preservam calor, produtos e tempo de vida, incluindo os dois valores de calor para ouro/chumbo conforme a configuração.
- O cálculo do tempo de vida reproduz a multiplicação float32 e as conversões para long do Java, com 48.000 ticks/dia e 100 dias/ano. A flag de decaimento é `enable528 || doRTGsDecay`; a escala de potência usa `enable528 || scaleRTGPower`. Defaults do fonte: decaimento ligado, escala de potência desligada. Configuração externa não foi aplicada ao snapshot.
- O calor de cada pellet é lido antes de decrementar/substituir o item. O campo `PELLET_DEPLETION` é por stack. O fonte inicializa um stack sem NBT com a vida completa; um compound existente sem esse campo lê zero. Isso está registrado para a lógica preservar.
- Radgen: 12 filas independentes, 12 saídas e buffer de 1.000.000 HE. A tabela contém 28 combustíveis reais, com potência por tick, duração e resíduo. Tiny tem 1/10 da potência e 1/10 da duração do tamanho normal, conforme o fonte. Sucata nuclear não deixa resíduo; a gema radioativa deixa diamante.
- O radgen continua processando com o buffer cheio e descarta a energia excedente. A capacidade da saída é verificada ao iniciar a fila; o fonte não a verifica novamente na conclusão. Slots, estado e filas persistidos, portas, redes e funcionamento nativo são trabalho separado.

Os pellets esgotados têm `setContainerItem(plate_iron)` no NTM. As cinco receitas literais de reciclagem devolveriam essa carcaça; o item de conteúdo atual não oferece esse retorno. Por isso permanecem como cinco gaps explícitos (`native crafting remainder required`), e o catálogo registra `crafting_remainder: ntm:plate_iron`. O caso de níquel já fica no ramo condicional do fonte. Não foi publicada uma reciclagem que perdesse a placa nem criada uma receita temporária.

O extrator de hazards passou a ler `registerRTGPellet`, o overload de Item para todas as metas dos resíduos novos e o ItemStack específico do pellet esgotado de neptúnio. As taxas são do NTM: por exemplo, pellet padrão 15 RAD/s, chumbo 15.000 RAD/s, neptúnio esgotado 3,75 RAD/s; resíduos curtos 30 RAD/s e longos 5 RAD/s, com os respectivos fatores de tamanho/decaimento. HOT/BLINDING do helper também são conservados como dados. Nenhuma taxa nem outro hazard de item já existente foi alterado (comparação por ID com a base C2).

A taxa RTG exportada é a base fresca. O `HazardModifierRTGRadiation` interpola até o alvo conforme a vida restante; os dez casos estão registrados como lacuna dinâmica para a camada nativa. O snapshot não apresenta a taxa base como dose correta para todo estado de decaimento.

Solução reaproveitada: catálogo de energia, parser de enums/registro, o escritor de snapshots existente, tradutor de receitas, extrator de hazards, machine-gen e o verificador em jogo. Nenhuma lógica de geração ou radiação Rust foi implementada neste lote.

Validação: 177 testes em cada seed 1/42 e os validadores de conteúdo e receitas passaram. A prova no ntm-next com bundle NTM/Recipes, em Xvfb e sem áudio, passou em 115 verificações: blocos nos níveis 0/5 e todos os 70 IDs recebidos e conferidos no inventário em lotes. Detector de rejeição/panic limpo. A captura confirma as texturas de fallback; não afirma funcionamento ou fidelidade dos multiblocos.

Evidências locais: `content-gen-evidence/energy-radioisotopes/`. Pacote persistente: `dist/ntm-content-energy5.hmod`, 6.641.553 bytes, SHA-256 `04b51c6e02a4be3336cf9630e96b2c5a6c00c6149c9000cce9abc0274e6edf29`.

O check do upstream e o do fork passaram no mesmo pacote. A comparação de hazards com a base C2 confirmou 70 entradas novas e zero alterações de taxas/efeitos antigos. O mundo descartável da prova foi removido; instalações e mundos reais não receberam o pacote.
