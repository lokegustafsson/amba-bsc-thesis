% Lokes råtext

# AMBA (funktionalitet)

Presenterar verktyget AMBA. Är byggt ovanpå S2E som är byggt ovanpå
QEMU. Verktyg för dynamisk analys för x86\_64 linux host, kan köra
x86\_64-linux-binärer med symbolisk indata. (eventuellt nämna att
input är argv, envp, stdin och filer. Endast stdin implementerat dock)
AMBA är ett grafisk användargränssnitt som i bakgrunden kör det
analyserade programmet i en (eller flera, om vi implementerar
multiprocessing) KVM-accelererade Ubuntu-22.04-x86\_64 gäster inuti
QEMU. Genom S2E kan register och minnesaddresser innehålla symboliska
uttryck, och vid symboliskt beroende hopp delas exekveringen upp i
separata spår.

Vid varje enskild tidpunkt har S2E utforskat en delmängd av alla
möjliga programbeteenden givet den symboliska indatan. Denna delmängd
visualiseras för användaren i form av en uppsättning grafer.

- Graf av symboliska tillstånd, defineras som minsta grafen med
    konkret PC. (förklara vad detta är). Relaterad metadata (som visas
    upp i nuvarande impl?) är symboliska uttryck och krav
    (hoppkrav). Syscalls (inte implementerat, kanske skippa)
- Graf av Basic Blocks (etablerad term, ha i ordlistan), defineras som
    minsta grafen där hopp endast avslutar noder. Grafen av symboliska
    tillstånd kan konstrueras genom kantkontraktion i
    Basic-Block-grafen. Relaterad metadata: address, assemblerkod,
    funktionsnamn, rad-kolumn-debugdata, syscalls.
- Komprimerad BB-graf, defineras som BB-graf där alla kanter som är
    unik utkant respektive inkant för två noder komprimeras. Möjliggör
    att visa större block assemblerkod i taget.

Grafnodernas placering i 2D hittas genom iterativ lösning för lägst
energi i ett system av attraktion-, repulsion- och externa
krafter. Utplaceringsalgoritmens parametrar kan konfigureras i realtid
vid körning av användaren.

Nånting nånting om tillståndsprioritering.

# Implementation

## Exekveringsmotor S2E/SymQemu/angr

nånting nånting

## Blah

## Mer Blah

# Slutsats / trevlig lista på funktionalitet att lägga till

- kontinuerlig sökning efter magiska strängar och tal i
    programminnet. Hitta ctf-flaggor och dylikt
- tidigare nämnda saker
