use axum::{Router, routing::{get, post}, response::{Json, Html}, http::StatusCode};
use serde::{Deserialize, Serialize};
// 1. Importar os componentes necessários do socketioxide
use socketioxide::{
    extract::{Data, SocketRef},
    SocketIo,
};
// Importar o CorsLayer
use tower_http::cors::{Any, CorsLayer};

mod blackjack;
use blackjack::sort_cards; 

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
        par_1: [pares[0], pares[1]],
        par_2: [pares[2], pares[3]],
    };
    Json(blackjack_init)
}

// 2. Definir os handlers para os eventos do socket.io
fn on_connect(socket: SocketRef, Data(data): Data<serde_json::Value>) {
    println!("Socket.IO conectado: {:?} {:?}", socket.ns(), socket.id);
    println!("Dados de autenticação: {:?}", data);

    // Handler para o evento "message"
    socket.on(
        "message",
        |socket: SocketRef, Data::<String>(data)| {
            println!("Recebido evento 'message': {}", data);
            socket.emit("response", &data).ok(); // Envia a mensagem de volta
        },
    );
}

#[derive(Serialize, Deserialize)]
struct Message {
    status: String,
    content: String,
}
#[derive(Serialize, Deserialize)]
struct BlackjackInit {
    par_1: [u8;2],
    par_2: [u8;2],
}

#[tokio::main]
async fn main() {
    // Opcional, mas recomendado para ver os logs do socket.io
    tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(tracing::Level::INFO)
        .init();

    // 3. Inicializar a camada do SocketIo e definir o handler de conexão
    let (layer, io) = SocketIo::new_layer();
    io.ns("/", on_connect);

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
        .layer(layer);
        
    let listener = tokio::net::TcpListener::bind("127.0.0.1:9000").await.unwrap();
    println!("Servidor rodando em:  http://{}", listener.local_addr().unwrap());

    //Envolve a aplicação inteira (incluindo socket.io) com a camada CORS
    axum::serve(listener, app.layer(cors)).await.unwrap();
}
