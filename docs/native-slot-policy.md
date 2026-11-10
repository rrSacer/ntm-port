# Política condicional de slots nativos

O hook existente `slot_filters` continua retornando `Vec<ModSlotFilter>`. O campo
opcional `policy` adiciona uma consulta pura por slot, executada no cliente e no
servidor pelo mesmo código Rust do bundle. Não muda o protocolo de cliques.

```rust
pub type SlotPolicyFn = fn(
    inventory: &[Option<ItemStack>],
    slot: usize,
    item: &ItemStack,
    action: SlotAction,
    origin: SlotOrigin,
) -> bool;

pub struct SlotPolicy(pub SlotPolicyFn);
pub enum SlotAction { Insert, Extract }
pub enum SlotOrigin { Manual, Automation }
// ModSlotFilter { ..., policy: Option<SlotPolicy> }
```

Os tipos são exportados por `horizon_client::containers`. `inventory` contém os
slots completos e atuais do contêiner, sem os 36 slots do jogador. `slot` é o
índice local, começando em zero. `item` conserva contagem, dano e componentes,
incluindo `minecraft:custom_data`. Na extração, é o item presente no slot; na
inserção, é o candidato à transferência. A consulta não confirma a transferência.

| Origem | Chamadores |
| --- | --- |
| `Manual` | Previsão da tela, cliques autoritativos, shift-click, teclas numéricas, descarte, coleta e drag. Shift-click permanece manual, mesmo que mova itens automaticamente dentro da tela. |
| `Automation` | Funil, funil de carrinho e outras máquinas que usam `Containers::can_place` / `can_take`. |

A consulta deve ser determinística, sem efeitos colaterais, acesso ao mundo,
relógio ou estado global mutável. Não capture um inventário anterior no hook
`slot_filters`: ele é coletado por tipo de bloco. Cada tentativa consulta o
inventário atual; o próximo slot de um drag ou transferência vê as inserções já
realizadas. A previsão pode ser corrigida pelo servidor ao receber seus slots.

`false` proíbe a operação; `true` permite continuar as verificações existentes.
Filtros `accept`/`reject`, `output`, limites de pilha, faces e listas
`ModContainer::inputs`/`outputs` continuam obrigatórios. `policy: None` conserva
o comportamento anterior. `Rules::may_pickup` e o evento oficial
`inventory_move_item` continuam ativos e também podem recusar operações.

### Troca direta: o slot é avaliado vazio

Quando um item **diferente** toma o lugar do que está no slot (clique com o
cursor ocupado, teclas 1–9 ou `F`), `may_pickup` aprova a retirada e a inserção
é consultada sobre uma visão em que o slot já está vazio; as demais posições
continuam sendo as atuais. Assim uma política como "só insere em slot vazio" não
exige dois cliques. Mesclar o **mesmo** item numa pilha existente continua vendo
o slot ocupado. Cliente (`Menu`) e hóspede LAN (`lan::menu`) aplicam a mesma
regra, inclusive o desdobramento da tecla numérica: se a pilha excede o limite
do slot, ele recebe até `max`, o resto fica na mão/hotbar e a pilha antiga vai
para o inventário (ou ao chão).

### Shift-click mais estreito que a mão

`ModSlotFilter::quick_move: Option<Vec<String>>` limita o que o **shift-click**
(`transferStackInSlot`) move para o slot, além de `accept`/`reject`/`policy`.
`None` não acrescenta restrição; o clique, a tecla numérica e o drag seguem só as
outras regras.
Um slot `output` que nomeia a lista também a recebe pelo shift-click: o `mergeItemStack` do 1.7.10 nunca
consultava `isItemValid`, então um shift-click podia terminar num `SlotTakeOnly`. A mão continua sem
colocar nada ali (`ModSlotFilter::quick_inserts`). Um contêiner Java que usa `Slot` vanilla com um
`transferStackInSlot` seletivo (como o ZIRNOX) declara `accept: None` e
`quick_move: Some(itens)`. O shift-click permanece de origem `Manual`.

Filtros estáticos devem expressar restrições comuns às duas origens. Por
exemplo, se RTG aceita qualquer item manualmente, não adicione um `accept`
estático que o limite a pellets: ponha a restrição da automação na política.

## Exemplo mínimo

```rust
use horizon_client::{
    containers::{ModSlotFilter, SlotAction, SlotOrigin, SlotPolicy},
    items::ItemStack,
};

fn policy(slots: &[Option<ItemStack>], slot: usize, item: &ItemStack,
          action: SlotAction, origin: SlotOrigin) -> bool {
    match (action, origin) {
        (SlotAction::Extract, SlotOrigin::Automation) =>
            slot < 24 && item.item.ends_with("_depleted"),
        (SlotAction::Insert, SlotOrigin::Automation) =>
            slots.get(slot).is_some_and(Option::is_none),
        _ => true,
    }
}

let rule = ModSlotFilter {
    max: Some(1),
    policy: Some(SlotPolicy(policy)),
    ..Default::default()
};
```

O sufixo do exemplo é apenas ilustrativo. O ZIRNOX usa sua tabela fonte por
`ntm_core::native::slot_allows` → `zirnox::can_extract`; isso mantém barras ativas,
incluindo combustível produzido pelo breeding, dentro do reator. Os slots 0..23
são acessíveis por automação; 24..27 continuam acessíveis somente pela tela.
Fiel ao `ContainerReactorZirnox.java`, os slots 0..23 são `Slot` vanilla: a
inserção **manual** aceita qualquer item (uma unidade por slot). A restrição a
barras vale na automação (`isItemValidForSlot`, em `slot_allows`) e no
shift-click (`quick_move`, `transferStackInSlot`).

## Integração RTG/radgen

A trilha RTG/radgen adicionará callbacks próprios em `ntm_glue::slot_filters`,
convertendo os stacks com a ponte existente e escolhendo as funções puras:

| Ação/origem | Função RTG/radgen |
| --- | --- |
| Inserção manual | `radioisotopes::can_insert_manual(block, slot)` |
| Inserção automática | `radioisotopes::can_insert(block, slots, slot, item)` |
| Extração automática | `radioisotopes::can_extract(block, slot)` |
| Extração manual | Permitida, respeitando as regras comuns do menu |

O RTG recusa inserção e extração automáticas. O radgen equilibra as 12 filas
somente em `Automation`. Essa ligação pertence à trilha RTG/radgen após a
integração do marco do hook, sem cherry-picks entre as branches.

## Portas de inventário de multiblocos

```json
{"multiblock": {
  "min": [-1,0,-2], "max": [1,0,3], "offset": 2, "dummy": "example:dummy",
  "inventory_ports": [[0,0,3], [1,0,0], [-1,0,0]]
}}
```

As coordenadas usam o quadro local existente: x à direita de quem coloca,
y acima, z para longe de quem coloca. `inventory_ports` é opcional; a ausência
significa que nenhum dummy expõe inventário. Cada entrada deve pertencer ao
volume e ser diferente do núcleo. Uma célula declarada resolve para o contêiner
atual do núcleo, com os mesmos slots, filtros, limites e origem `Automation`.
Não cria um BE separado nem copia os itens para o proxy. Núcleo inacessível ou
sem contêiner recusa a operação. A orientação da face é encaminhada como na
consulta ao núcleo. O evento oficial de transferência identifica o contêiner
real do núcleo.

`Containers::view`, usado pelo funil, carrinho e `native_insert_item`, faz essa
resolução. A interação manual e a LAN continuam abrindo o menu do núcleo pelo
mecanismo de multiblocos existente; sua origem permanece `Manual`.

O ZIRNOX declara as cinco portas Java: `[2,1,0]`, `[2,3,0]`, `[-2,1,0]`,
`[-2,3,0]` e `[0,4,0]`. Um funil vanilla insere pela porta superior. Todas
as portas laterais têm outra célula estrutural logo abaixo: um funil extrator
sob essas portas ocuparia o invólucro. Por isso a prova do ZIRNOX extrai pelo
núcleo inferior, sem remover cascas. Essa geometria também impede a instalação
no Java; não é uma limitação da resolução de portas do port.

O cenário `run-native-reactors.py inventory-proxy` cria um hmod descartável que
altera somente a forma do ZIRNOX para a caixa de três portas acima, mantendo
seu contêiner e sua política. Ele carrega barras pela tela normal, converte uma
barra de lítio em trítio com `/tick sprint` e verifica transferência real por
funis sobre e sob a porta lateral, recusa na célula não-porta e contagem exata
dos itens.

### Pedido ao motor: lista de slots por célula (soldadora a arco)

A política de slot recebe slot, item, ação e origem, mas **não a célula** pela qual o funil chega, e o contêiner é um só por
máquina. O Java dá a cada célula da soldadora a sua lista (`TileEntityMachineArcWelder.getAccessibleSlotsFromSide(x,y,z,side)`,
linhas 312-331): o núcleo e a célula "amarela" acessam o slot 1, as "vermelhas" o 0, as "verdes" o 2, e a saída (3) sai por todas;
as seis células da camada de cima não dão nada. O port expõe as três entradas e a saída em cada célula-porta e tirou as seis de
cima de `inventory_ports`. Para reproduzir as listas por célula, `ModSlotFilter::policy` (ou o contêiner) precisa receber a posição
da célula relativa ao núcleo. Registrado como lacuna nominal em `docs/onda-2-sublote-1.md`.
