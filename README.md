# `szaklon-api`

Serwer backendu/API projektu Szaklon.

## Instalacja Rusta

Instrukcje znajdują się na stronie https://www.rust-lang.org/tools/install.

Można też zainstalować rusta używając menadżera pakietów, ale o ile nie korzystasz
z Archlinuxa lub pochodnych wersja w repo może być zbyt stara (testuję tylko na 1.34+).

## Uruchomienie

`cargo run --release`

Przy pierwszym uruchomieniu cargo pobierze wszystkie zależności i je skompiluje co może
potrfać trochę czasu. Flaga `--release` włącza optymalizacje, zalecam jej użycie, aby
nie czekać godziny na policzenie hasha hasła.

**Aplikacaj wymaga sqlite**. Jest to biblioteka napisana w C, więc są pewne niedogodnosci.
Jeśli korzystasz z Linuxa, po prostu zainstaluj korzystając z menadżera pakietów. Jeśłi
korzystasz z Windowsa, kompiluj z flagą `--features bundled`. Wymusi to na skomilowanie
i użycie wersji sqlite dostarczanej wraz biblioteką definiującą interfejs.

Aby zmienić parametry aplikacji, zobacz plik `config.toml`.

## Migracje bazy danych

Dla deweloperów: zobacz początek poradnika: http://diesel.rs/guides/getting-started/  
tj. zainstaluj i uruchom diesel_cli:
```
cargo install diesel_cli
DATABASE_URL=db.sqlite diesel setup
```

Dla innych: Pobierz najnowszy plik z #1

## Docker

Instrukcje i dodatkowe informacje jak uruchomić backend przy pomocy Dockera.

Zbuduj obraz:

```
docker build -t szaklon-backend .
```

Aplikacja skopiuje plik `config-prod.toml` do kontenera. Aby dodać TLSa, należy przed
zbudowaniem edytować plik konfiguracyjny oraz Dockerfile i odkomentować odpowiednie
linie.

Kontener do działania potrzebuje zamontowanego wewnątrz katalogu `/app` z plikiem
`db.sqlite`. Zadaniem dla Azurowców jest wymyślenie jak zrobić, żeby to działało
na Azurze, prawdopodobnie przyda się https://docs.docker.com/docker-for-azure/persistent-data-volumes/.
Dla testów lokalnych można wykonać poniższe polecenia:

```
docker volume create szaklon-db
docker run -v szaklon-db:/data --name helper busybox true
docker cp db.sqlite helper:/data
docker rm helper
```

Zostało uruchomienie. Ważne jest, aby „opublikować” port z kontenera na zewnątrz. Należy do
tego użyć flagi `-p` (przy okazji przypominam, że https jest serwowany na innym porcie!):

```
docker run -p 9876:9876 -v szaklon-db:/app -t szaklon-backend
```

Zostało tylko sprawdzić, czy serwer działa:

```
curl 127.0.0.1:9876/popular/5
```

Zapytanie powinno zwrócić tablicę z utworami (będzie pusta, w przypadku pustej bazy).

## Dokumentacja

`cargo doc --no-deps --open`

Znajduje się tam również dokumentacja API HTTP. 
