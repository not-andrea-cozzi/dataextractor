use std::io::{self, Read};
use std::path::Path;
use std::sync::mpsc::{self, Receiver};

use pgn_reader::Reader;
use tokio::io::{AsyncReadExt, BufReader};
use tokio::task;

use crate::graph::{build_game_graph, GameGraph};
use crate::pgn_visitor::GameVisitor;

const CHUNK_SIZE: usize = 64 * 1024;

/// Read adapter: espone come `Read` i chunk prodotti dal task async.
struct ChunkReader {
    rx: Receiver<anyhow::Result<Vec<u8>>>,
    current: Vec<u8>,
    position: usize,
}

impl ChunkReader {
    fn new(rx: Receiver<anyhow::Result<Vec<u8>>>) -> Self {
        Self {
            rx,
            current: Vec::new(),
            position: 0,
        }
    }
}

impl Read for ChunkReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        loop {
            // Abbiamo ancora dati nel chunk corrente.
            if self.position < self.current.len() {
                let available = &self.current[self.position..];

                let count = available.len().min(buf.len());

                buf[..count].copy_from_slice(&available[..count]);
                self.position += count;

                return Ok(count);
            }

            // Chunk esaurito: prendine uno nuovo.
            match self.rx.recv() {
                Ok(Ok(chunk)) => {
                    self.current = chunk;
                    self.position = 0;

                    // Chunk vuoto: continuiamo.
                    if self.current.is_empty() {
                        continue;
                    }
                }

                Ok(Err(err)) => {
                    return Err(io::Error::new(
                        io::ErrorKind::Other,
                        err.to_string(),
                    ));
                }

                // Il producer ha terminato.
                Err(_) => {
                    return Ok(0);
                }
            }
        }
    }
}

fn parse_pgn_reader<R: Read>(
    reader: R,
    min_ply: usize,
) -> anyhow::Result<Vec<GameGraph>> {
    let mut reader = Reader::new(reader);
    let mut graphs = Vec::new();

    loop {
        let mut visitor = GameVisitor;

        match reader.read_game(&mut visitor)? {
            None => break,

            Some(None) => {
            
                continue;
            }

            Some(Some(game_state)) => {
                if game_state.records.len() < min_ply {
                    continue;
                }

                // TODO: mate in 1-5
                let mate_in = None;

                let graph =
                    build_game_graph(&game_state.records, mate_in);

                graphs.push(graph);
            }
        }
    }

    Ok(graphs)
}


pub async fn parse_pgn_file_chunked_async(
    path: impl AsRef<Path>,
    min_ply: usize,
) -> anyhow::Result<Vec<GameGraph>> {
    let path = path.as_ref().to_path_buf();

    let file = tokio::fs::File::open(&path).await?;
    let mut file = BufReader::new(file);

    let (tx, rx) = mpsc::sync_channel::<anyhow::Result<Vec<u8>>>(4);

    // Task che legge il file asincronamente.
    let reader_task = tokio::spawn(async move {
        loop {
            let mut buffer = vec![0u8; CHUNK_SIZE];

            let n = file.read(&mut buffer).await?;

            if n == 0 {
                break;
            }

            buffer.truncate(n);

            // `blocking_send` è importante perché il receiver
            // è usato dal parser sincrono.
            if tx.blocking_send(Ok(buffer)).is_err() {
                break;
            }
        }

        Ok::<(), anyhow::Error>(())
    });

    // Il parser PGN è sincrono, quindi lo eseguiamo su un
    // thread blocking di Tokio.
    let parse_task = task::spawn_blocking(move || {
        let reader = ChunkReader::new(rx);

        parse_pgn_reader(reader, min_ply)
    });

    let graphs = parse_task.await??;

    reader_task.await??;

    Ok(graphs)
}