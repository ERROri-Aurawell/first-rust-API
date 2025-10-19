use axum::{http::StatusCode, response::{Html, Json}, routing::{get, post}, Router};
use serde::{Deserialize, Serialize};
// 1. Importar os componentes necessários do socketioxide
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
// Importar o CorsLayer
use tower_http::cors::{Any, CorsLayer};
use std::sync::{Arc, Mutex};

mod blackjack;
use blackjack::{sort_cards, create_room};

async fn root() -> Html<&'static str> {
    Html("<h1>Olá, mundo! Esta é a minha primeira API com Axum.</h1>\n <h1>Bem-vindo!</h1>")
}

async fn message() -> Json<Message> {
    let msg = Message {
        status: "success".to_string(),
        content: "Esta é uma mensagem JSON.".to_string(),
    };
    Json(msg)
}

async fn input_message(Json(payload): Json<Message>) -> (StatusCode, Json<Message>) {
    //Nenhuma validação é nescessária. O desserialize já cuida disso.
    let status = if payload.status != "success" {
        "error".to_string()
    } else {
        "received".to_string()
    };
    let response = Message {
        status,
        content: format!("Mensagem recebida: {}", payload.content),
    };
    
    if response.status == "error" {
        return (StatusCode::BAD_REQUEST, Json(response));
    }else{
        (StatusCode::OK, Json(response))
    }
}

async fn black_init() -> Json<BlackjackInit> {
    let mut total_deck: Vec<u8>  = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    let pares: [u8; 4] = sort_cards(&mut total_deck);
    let blackjack_init = BlackjackInit {
        par_1: vec![pares[0], pares[1]],
        par_2: vec![pares[2], pares[3]],
        restante: total_deck,
    };
    Json(blackjack_init)
}

// Definimos o estado da aplicação.
// `#[derive(Clone)]` é necessário para que o Axum possa compartilhar o estado entre os handlers.
#[derive(Clone)]
struct AppState {
    rooms: Arc<Mutex<Vec<Room>>>,
}

// Payload para mensagens genéricas
#[derive(Serialize, Deserialize)]
struct Message {
    status: String,
    content: String,
}
#[derive(Serialize, Deserialize)]
struct BlackjackInit {
    par_1: Vec<u8>,
    par_2: Vec<u8>,
    restante: Vec<u8>,
}
#[derive(Debug)] // Adicionado para permitir a impressão com {:?}
struct Room{
    id: String,
    name: String,
    player1_id: String,
    player2_id: Option<String>,
    deck: Vec<u8>,
    deck_1: Vec<u8>,
    deck_2: Vec<u8>,
}

// Payload para o evento Join_Room
#[derive(Deserialize, Debug)]
struct JoinRoomPayload {
    room_id: String,
}

#[tokio::main]
async fn main() {
    // Define o estado compartilhado da aplicação
    let state = AppState {
        rooms: Arc::new(Mutex::new(Vec::new())), // 1. Envolvemos o Mutex em um Arc
    };

    // Opcional, mas recomendado para ver os logs do socket.io
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .init();

    // 3. Inicializar a camada do SocketIo e definir o handler de conexão
    let (layer, io) = SocketIo::new_layer();

    // Usamos uma closure para o handler de conexão que captura o `state`.
    // O `move` é crucial para que a closure tome posse do `state.clone()`.
    let state_clone = state.clone();
    io.clone().ns("/", move |socket: SocketRef, Data(data): Data<serde_json::Value>| {
        println!("Socket.IO conectado: {:?} {:?}", socket.ns(), socket.id);
        println!("Dados de autenticação: {:?}", data);

        // Handler para o evento "message"
        socket.on(
            "message",
            |socket: SocketRef, Data::<String>(data)| {
                println!("Recebido evento 'message': {}", data);
                let response = format!("Olá {}, recebi seu {}",&socket.id, &data);
                socket.emit("response",&response).ok(); // Envia a mensagem de volta
            },
        );

        // O `state_clone` capturado agora está disponível aqui.
        let state_for_room = state_clone.clone();
        let io_for_create = io.clone();
        socket.on(
            "Create_Room",
            // A closure agora é `async move` para permitir o uso de `.await`
            move |socket: SocketRef, Data::<String>(name)| async move {
                let player1_id = socket.id.to_string();
                let room_id = create_room(&player1_id);
                println!("Jogador {} criou a sala com ID: {}", player1_id, room_id);

                let decks = {
                        let mut total_deck: Vec<u8>  = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
    let pares: [u8; 4] = sort_cards(&mut total_deck);
    let blackjack_init = BlackjackInit {
        par_1: vec![pares[0], pares[1]],
        par_2: vec![pares[2], pares[3]],
        restante: total_deck,
    };
    blackjack_init
                };

                let new_room = Room { id: room_id.clone(), player1_id, player2_id: None, deck: decks.restante, deck_1: decks.par_1, deck_2: decks.par_2, name };
                
                // Adiciona o socket do criador à sala do socket.io
                let _ = socket.join(room_id.clone());

                // Bloqueia o Mutex para adicionar a nova sala
                state_for_room.rooms.lock().unwrap().push(new_room);
                socket.emit("Room_Created", &room_id).ok(); // Envia o ID da sala de volta
                let response = format!("{:?}", state_for_room.rooms.lock().unwrap());
                io_for_create.to(room_id).emit("Room_Ready", &response  ).await.ok();
            });

        // Handler para o evento "Join_Room"
        let state_for_join = state_clone.clone();
        let io_for_join = io.clone(); // Clonamos o `io` para poder emitir para a sala
        socket.on(
            "Join_Room",
            // 1. A closure agora é `async move` para permitir o uso de `.await`
            move |socket: SocketRef, Data::<JoinRoomPayload>(payload)| async move {
                println!("Jogador {} tentando entrar na sala {}", socket.id, payload.room_id);
                
                // 2. O bloqueio do Mutex é feito dentro de um escopo próprio `{...}`
                //    Isso garante que o `lock` seja liberado antes de qualquer `.await`.
                let mut joined = false;
                { // Início do escopo do lock
                    let mut rooms = state_for_join.rooms.lock().unwrap();
                    if let Some(room) = rooms.iter_mut().find(|r| r.id == payload.room_id) {
                        if room.player2_id.is_none() && room.player1_id != socket.id.to_string() {
                            room.player2_id = Some(socket.id.to_string());
                            println!("Jogador {} entrou na sala {}", socket.id, payload.room_id);
                            let _ = socket.join(payload.room_id.clone());
                            joined = true;
                        }
                    }
                } // Fim do escopo do lock, `rooms` é liberado aqui.

                if joined {
                    // 3. A chamada `.await` agora é feita fora do escopo do lock.
                    io_for_join.to(payload.room_id).emit("Player_Joined", &format!("Jogador {} entrou. O jogo pode começar!", socket.id)).await.ok();
                } else {
                    // Estas chamadas `emit` não precisam de `await` se não nos importarmos com o resultado, mas é uma boa prática.
                    if state_for_join.rooms.lock().unwrap().iter().any(|r| r.id == payload.room_id) {
                        socket.emit("Join_Error", "A sala está cheia ou você é o criador.").ok();
                    }
                    else {
                        socket.emit("Join_Error", "Sala não encontrada.").ok();
                    }
                }
            });
    });

    // Criar uma camada CORS permissiva para desenvolvimento
    let cors = CorsLayer::new()
        .allow_origin(Any) // Permite qualquer origem
        .allow_methods(Any) // Permite qualquer método (GET, POST, etc.)
        .allow_headers(Any); // Permite qualquer cabeçalho

    // Aplica a camada do socket.io ao router principal
    let app = Router::new()
        .route("/", get(root))
        .route("/message", get(message))
        .route("/post_message", post(input_message))
        .route("/blackjack_init", get(black_init))
        .layer(layer)
        .with_state(state); // Disponibiliza o estado para os handlers
        
    let listener = tokio::net::TcpListener::bind("127.0.0.1:9000").await.unwrap();
    println!("Servidor rodando em:  http://{}", listener.local_addr().unwrap());

    //Envolve a aplicação inteira (incluindo socket.io) com a camada CORS
    axum::serve(listener, app.layer(cors)).await.unwrap();
}
