# Reatores R4: Watz

Entram 29 IDs, levando o catálogo a 2.991 (509 blocos e 2.482 itens). Só conteúdo: o reator, a bomba e o núcleo estrutural entram como Inert no registro nativo. Estende o R2/R3 (mesmos geradores, extrator de hazards e perfil de prova). Já existiam `watz_element`, `watz_cooler` e os recipientes de fluido WATZ.

| Grupo | IDs (prefixo `ntm:`) |
| --- | --- |
| Máquinas (3) | `watz`, `watz_pump`, `struct_watz_core` (beehive, facing, honey_level 0..5, `auto: false`; o fallback de textura é o aço do fonte e a textura do núcleo) |
| Carcaça (2) | `watz_end` (meta 0) e `watz_end__bolted` (meta 1; o fonte chama a variante `watz_end_bolted`). Cubos simples com `watz_casing`/`watz_casing_bolted` |
| Pellets (12) | `watz_pellet__<schrabidium, hes, mes, les, hen, meu, mep, lead, boron, du, nqd, nqr>` |
| Pellets esgotados (12) | `watz_pellet_depleted__<os mesmos>` |

`struct_watz_core` entra porque é a peça atual do reator: o Watz completo a dropa ao quebrar e ela tem receita no fonte; o `BlockToolConversion` da carcaça (chave de fenda/parafusadora) é lógica nativa, então só as duas formas como blocos simples. Os ícones dos pellets são o `watz_pellet.png` recolorido pela mesma remapeação RGB do fonte (`colorLight`/`colorDark`); os esgotados usam `desaturate` replicado do `ItemWatzPellet`. Pilha de 16, como no fonte. A `watz_pellet_schrabidium.png` solta não é usada pelo fonte.

**Tabela do fonte** (`content/machine-recipes/reactors_watz.json`): para cada tipo, cores, fluxo passivo, calor por fluxo, conteúdo de lama (valor do construtor dividido por 2), função de queima, divisor de calor e função de absorção, o rendimento 500.000.000 e o papel (nove combustíveis, três absorvedores: chumbo, boro e urânio empobrecido); os tanques do reator (COOLANT recebe; COOLANT_HOT e WATZ saem; mesmos três para o conjunto compartilhado, 64.000 mB); o que o Watz completo dropa ao quebrar. Reação, calor, tanques compartilhados, bomba, GUI, falha e a conversão da carcaça são trabalho nativo; vida do pellet é NBT.

**Receitas originais:** 10 pellets (`RodRecipes.addPellet`, anel de quatro lingotes com um lingote de grafite no centro, as duas sobrecargas validadas contra o corpo do helper; as variantes Naquadah/Naquadria dependem de OreDict de GT6 e ficam de fora); montadora 462 (+`ass.watzcasing`); PUREX de 27 para 37 operações (as dez reciclagens de Watz cujo `watz_pellet_depleted` faltava; as duas de Naquadah/Naquadria continuam em skipped porque `nuggetNaquadria` não existe no catálogo). A bomba e o núcleo estrutural continuam sem receita por usarem `ANY_RESISTANTALLOY.plateCast()` ainda não resolvido. Bigorna e demais tabelas não mudam.

**Hazards.** O extrator passa a ler `HazardSystem.register(DictFrame.fromOne(ModItems.watz_pellet, EnumWatzType.X), ...)`: dez pellets frescos radioativos; os esgotados não têm registro no fonte e ficam em zero. Nenhuma taxa existente mudou.

**Prova e validação.** `verify-bedrock-processing.py --profile reactors-r4`: 29 identidades, 91 conferências (três máquinas e quatro blocos em dois níveis), sem `reject` nem `panic` no log do cliente nativo. 200 testes por seed 1/42, `validate-bulk` e `recipe-gen/validate` verdes. Pacote `ntm.hmod` SHA-256 `f01fced017e8481f04c9f1d23b9973ed4ecab8eca9b28b2768ae875c17008835` (sobre o R3), aprovado no check do upstream e do fork.
