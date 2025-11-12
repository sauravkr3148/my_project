package relay

import "github.com/gofiber/contrib/websocket"

type websocketRelay struct {
	agent_receiver  chan []byte
	client_receiver chan []byte

	agent_status bool

	screen_transfer bool
	clients         map[string]*websocket.Conn
}

type AgentKeyChain struct {
	Key       string `json:"key"`
	PublicKey string `json:"publickey"`
}

type UserAgentPermission struct {
	Devices string `json:"Devices"`
}

type RegistrationForm struct {
	FullName string `form:"fullName" json:"fullName"`
	Email    string `form:"email" json:"email"`
	Phone    string `form:"phone" json:"phone"`
}

type ChatMessage struct {
	Type      string `json:"type"`
	From      string `json:"from"`
	To        string `json:"to"`
	Message   string `json:"message"`
	Timestamp int64  `json:"timestamp"`
	AgentType string `json:"agent_type"`
}

type StatusMessage struct {
	Type      string `json:"type"`
	AgentID   string `json:"agent_id"`
	Status    string `json:"status"`
	AgentType string `json:"agent_type"`
}
