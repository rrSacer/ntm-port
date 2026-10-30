# Andaimes coloridos e estados vanilla da grade

A coordenação com o visual-gen adotou uma solução compatível com content API 0.2. O motor atual não declara propriedades novas para um bloco de conteúdo; seus estados vêm do template vanilla. Não foram alterados registros, protocolo ou saves do motor.

`ntm:steel_scaffold` conserva o ID original e passa a representar metadata 0/steel. As cores da fonte ganham `ntm:steel_scaffold__red` (1), `__white` (2) e `__yellow` (3), com as quatro texturas NTM e nomes en_us/pt_br. Todos usam **minecraft:quartz_pillar**, com estados axis=x/y/z. A colocação por face é a do pilar vanilla; a quarta orientação vertical do NTM, dependente de yaw, fica como lacuna, assim como suas colisões e interações customizadas.

As quatro receitas de tingimento da bancada foram resolvidas por metadata (cinza, vermelho, branco e amarelo); cada uma conserva oito andaimes como saída. A fabricação-base e as duas receitas de viga mantêm seus IDs. Há 909 receitas vanilla e 729 receitas exportadas de triturador; os novos três subtipos explicam o aumento. O catálogo passa a 2.656 IDs (392 blocos, 2.264 itens, 52 máquinas), com hazards para as novas cores.

`ntm:steel_grate` usa **minecraft:stone_slab**, com estados type=bottom/top/double e waterlogged herdado. O loot já trata double como duas unidades. A fonte usa uma placa de 2 px com dez alturas escolhidas por clique/sneak; essas alturas e a colisão fina não estão implementadas. O template de laje aproxima a colocação e reduz a colisão do antigo cubo, com comportamento vanilla explicitamente conservado.

Os blockstates/modelos de fallback acompanham esses estados. O visual-gen pode reivindicá-los em seu manifesto e desenhar a geometria original em cada variante; o content-gen respeita essa propriedade. Esse acordo não significa que a sobreposição visual ou sua aceitação em jogo já tenha sido concluída.

Passaram **111 testes Python em cada seed, PYTHONHASHSEED=1 e 42**, incluindo quatro testes das estruturas, além dos validadores de conteúdo e receitas. A validação cobre IDs/metadata, templates, eixos, receitas distintas de cor e hazards. Build/check API 0.2 passou: **5.528.814 bytes**, SHA-256 `79d89dc487048f6e67ae1c48cb808e344ffe8e1ec97a2d7a9c08f7f4157db125`. O HEAD isolado `066d491` passou nos **250 testes de ntm-core --lib**, e hazard-gen reproduziu os hashes de hazards/buckets. Evidências em `content-gen-evidence/structures/`; os temporários grandes ficam no disco em `.agent-tmp`.
