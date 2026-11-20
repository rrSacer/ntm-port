# Onda 2 — as fábricas (Grupo C)

Branch `onda2-k3-c`. `machine_chemical_factory`, `machine_assembly_factory` e `machine_precass`, avaliadas depois do ralo, do micro-ondas e das chaminés (`docs/onda-2-sumidouros.md`).

| # | Bloco | Estado |
| --- | --- | --- |
| 1 | `machine_chemical_factory` | **Núcleo pronto** (módulos, refrigerante, upgrades, slots gravados); faltam o multibloco com modelo, as portas e o painel (próxima mudança) |
| 2 | `machine_assembly_factory` | Pendente (mesmo desenho, quatro montadoras) |
| 3 | `machine_precass` | Bloqueada por `ntm:orbital_assembly` / perfil 528 (`docs/onda-2-sublote-1.md`, decisão registrada) |

---

## 1. `ntm:machine_chemical_factory` (núcleo)

### Contrato fonte

- `TileEntityMachineChemicalFactory.java` (560 linhas), `ModuleMachineBase.java` e `ModuleMachineChemplant.java`: **quatro módulos de planta química** em um bloco, 32 slots (bateria no 0, três upgrades nos 1 a 3 e, por módulo `i`, o slot de projeto em `4 + 7i`, três entradas em `5 + 7i` e três saídas em `8 + 7i`), 12 tanques de entrada e 12 de saída de 24 000 mB (três de cada por módulo), água (entrada) e vapor gasto (saída) de 4 000 mB, buffer de
  `max(power, soma de receita.power * 100, 1 000 000)` HE.
- `ModuleMachineBase.update(speed, power, extra, blueprint)`: os tanques do módulo tomam o tipo dos fluidos da receita por posição e os que a receita não usa são **zerados** (`resetTank`); com a condição extra (`canCool`: 100 mB de água e espaço para 100 de vapor gasto) e `canProcess` (energia `receita.power * pow`, entradas por posição com o tamanho da pilha, tanques de entrada, espaço nas saídas e nos tanques) o módulo gasta
  `receita.power * pow` HE e `progress += min(speed / duração, 1)` (um `double`); a 1 consome as entradas, faz as saídas (`ChanceOutput` por sorteio) e continua se ainda puder. Sem condição ou sem `canProcess` o progresso volta a 0. A troca automática de receita (`autoSwitchGroup`) olha a entrada 0.
- `updateEntity` da fábrica: carrega a bateria do slot 0 (`chargeTEFromItems`), lê os upgrades (velocidade, potência e overdrive, cada um até o nível 3): `speed = 1 + vel/3 + overdrive`, `pow = 1 - pot * 0,25 + vel + overdrive * 10/3`; cada módulo roda com `speed * 2` e `pow * 2`; **cada módulo que processou gasta 100 mB de água e dá 100 mB de vapor gasto**; depois, um tanque de entrada de um fluido tipado toma até 50 mB por tick de um tanque de saída do mesmo fluido e da mesma pressão.

### Implementação

- [`crates/ntm-core/src/chemfactory.rs`](../crates/ntm-core/src/chemfactory.rs) (`Kind::ChemFactory`, `Data::ChemFactory`): o modelo **grava os 32 slots** (sem `serde(skip)`: a restauração pelo journal já os traz; a classe de defeito do P1 da Fundição não se aplica), os quatro módulos (`recipe`, `progress`), os 24 tanques, a água e o vapor gasto; `step` é o `updateEntity`; consumidor de energia e `FluidNode` (as entradas e a água recebem; as saídas e o vapor gasto fornecem). As receitas são as da planta química do port (`data/machines/chemical_plant.json`).
- Ligada ao registro (`registry`), aos 32 slots nativos (`native::container`, `slot_filters`: as saídas só se retiram; `container_bridge::SLOTTED`) e ao `Slotted`. O bloco ainda é a "carcaça inerte" visual: **sem multibloco, modelo, portas nem tela**.

### Provas

- **Testes (`ntm-core`):** `chemfactory::tests` (16) e `server::tests::chemfactory_tests` (3): a disposição dos 32 slots, o buffer dinâmico (e o piso de 1 000 000), os upgrades (níveis e as duas fórmulas), uma receita termina no **11º tick**
  (0,1 somado dez vezes em `double` dá 0,9999999999999999, como no Java) com 100 mB de água por tick de trabalho e `receita.power * 2` HE, sem refrigerante (99 mB, ou sem espaço para o vapor) ou sem energia não anda (e a energia exata de um tick basta), o progresso zera sem as entradas, saída cheia e tanque de saída com 499 de 500 mB param,
  os tanques tomam o tipo da receita e os não usados são zerados, o compartilhamento de 50 mB por tick (e não com outra pressão ou outro fluido), a bateria carrega e o item fica, a troca automática (quando houver grupo), o modelo se grava inteiro; em servidor: os 32 slots do motor e um módulo fazendo duas rodadas de areia betuminosa sobre eles (com a água reposta, 4 000 mB pagam 40 ticks), a restauração do registro com os slots, as saídas só de retirar e as faces com as 24 posições de itens.
- **Mutantes (`mutate_chemfactory.py`, log `mutate_chemfactory.log`):** **30/32 mortos** (o primeiro passe matou 22; os sobreviventes `F04 F05 F10 F23 F28 F29 F30 F31` ganharam testes). Os dois restantes não são testáveis com os dados do port: `F24` (sorteio de chance invertido) e `F25` (troca automática que ignora o grupo), porque nenhuma receita de `chemical_plant.json` tem chance menor que 1 nem `autoSwitchGroup`.
- Sem prova em jogo ainda: o bloco não tem modelo nem multibloco (o painel e as portas vêm na mudança seguinte, com a prova em jogo e o corte estrito).

### Lacunas nominais

- **Projetos (blueprint pools):** uma receita de uma "piscina" exige o projeto certo no slot do módulo; as tabelas do port não têm piscinas (como na planta química isolada), então o slot de projeto não filtra nada.
- **Pressão dos fluidos de receita** (`FluidStack.pressure`): as tabelas não a trazem; os tanques ficam em pressão 0.
- **Regras de itens do funil** (`isItemValidForSlot` por receita, `canExtractItem` dos slots entupidos, os acessos por porta): por ora as 24 posições de itens são iguais para todas as faces e as saídas só se retiram; o refinamento por origem vem com o painel.
- Modo restrito (RoR), o som e a animação.
