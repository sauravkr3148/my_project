package relay

import "github.com/gofiber/contrib/websocket"

type RelayServer interface {
	agentPause(conn chan []byte)
	agentUnpause(conn chan []byte)
	SendConnectedClients()
	GenerateRandomKey() string
	RemoveClient(tenent_id string, key string, index int, agentType string) // Add agentType parameter
	RemoveReverseClient(tenent_id string, key string, agentType string)
	registerClient(key string, conn *websocket.Conn, tenant_id string, agentType string)
	registerReverseClient(key string, conn chan []byte, tenant_id string, agentType string)
	sendCurrentAgentStatuses(conn *websocket.Conn, tenant_id string, agentType string)
	CreateReverseClient(key string, conn *websocket.Conn, tenent_id string, agentType string)
	createClient(key string, conn *websocket.Conn, tenent_id string, agentType string)
	handleNewConnection(c *websocket.Conn)
	initHandlers() error
	Initialize()
	broadcastAgentOnlineStatus(tenent_id string, agentKey string, agentType string)
}
