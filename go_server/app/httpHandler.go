package relay

import (
	"cpc/gbl"
	"encoding/json"
	"fmt"
	"log"
	"net"
	"time"

	"github.com/gofiber/contrib/websocket"
	"github.com/gofiber/fiber/v2"
	"github.com/gofiber/fiber/v2/middleware/session"
	"github.com/gofiber/template/html/v2"
)

func NewWebsocketRelayServer() websocketRelay {
	var agent_receiver = make(chan []byte)
	var client_receiver = make(chan []byte)
	var agent_status = true
	var screen_transfer = true
	var clients = make(map[string]*websocket.Conn)
	return websocketRelay{agent_receiver, client_receiver, agent_status, screen_transfer, clients}
}

var store = session.New()

func initHandlers() *fiber.App {
	engine := html.New("./views", ".html")
	app := fiber.New(fiber.Config{
		Views: engine,
	})
	app.Static("/", "./public")

	app.Use("/ws", func(c *fiber.Ctx) error {

		if websocket.IsWebSocketUpgrade(c) {
			c.Locals("allowed", true)
			return c.Next()
		}
		return fiber.ErrUpgradeRequired
	})
	return app
}

func printRemoteIP(wsConn *websocket.Conn) {
	netConn := wsConn.UnderlyingConn()

	remoteAddr := netConn.RemoteAddr().String()

	DebugPrintln("Remote address:", remoteAddr)
	host, _, err := net.SplitHostPort(remoteAddr)
	if err != nil {
		DebugPrintln("Error parsing remote address:", err)
		return
	}

	fmt.Println("Remote IP:", host)
}
func (w websocketRelay) initHandlers() error {
	app := initHandlers()
	app.Get("/ws/:type/:agent_type/:tenant_id/:key", websocket.New(func(c *websocket.Conn) {
		printRemoteIP(c)
		w.handleNewConnection(c)
	}))

	app.Get("/remote", func(c *fiber.Ctx) error {
		return c.Render("remote", fiber.Map{
			"Title":   "Hello, World!",
			"Devices": GetAllowedDeviceMap(c),
		})
	})
	app.Get("/admin", func(c *fiber.Ctx) error {
		users, keys := getAllUsersAndKey()
		return c.Render("admin", fiber.Map{
			"keys":  users,
			"users": keys,
		})

	})

	app.Post("/admin/:action", func(c *fiber.Ctx) error {
		action := c.Params("action")
		msg := ""

		users, keys := getAllUsersAndKey()

		switch action {

		case "register":
			msg = registerUser(c, w)

		case "assign-device":
			msg = assignDevice(c, w)
		}
		return c.Render("admin", fiber.Map{
			"keys":  users,
			"Msg":   msg,
			"users": keys,
		})
	})

	app.Get("/login/:token", func(c *fiber.Ctx) error {

		sess, err := store.Get(c)
		if err != nil {
			return err
		}
		sess.Set("authToken", "6f50a8495a6ece1251dbeb3ce1ad748e")
		return c.Redirect("/remote")
	})

	api := app.Group("/api/v1")

	api.Post("/files/list/:agent_type/:tenant_id/:key", w.handleListFiles)

	api.Post("/files/upload/:agent_type/:tenant_id/:key", w.handleFileUpload)

	api.Get("/files/download/:agent_type/:tenant_id/:key", w.handleFileDownload)

	api.Delete("/files/delete/:agent_type/:tenant_id/:key", w.handleFileDelete)

	api.Post("/files/mkdir/:agent_type/:tenant_id/:key", w.handleCreateDirectory)

	api.Put("/files/rename/:agent_type/:tenant_id/:key", w.handleFileRename)

	api.Get("/agent/details/:agent_type/:tenant_id/:key", w.handleGetAgentDetails)

	api.Get("/agents/keys/:tenant_id", w.handleGetAgentKeys)

	api.Get("/agent/software/:agent_type/:tenant_id/:key", w.handleGetInstalledSoftware)

	api.Post("/files/operation/:agent_type/:tenant_id/:key", w.handleFileOperation)

	api.Post("/encoder/settings/:agent_type/:tenant_id/:key", w.handleEncoderSettings)

	api.Post("/files/edit/:agent_type/:tenant_id/:key", w.handleFileEdit)

	api.Post("/files/save/:agent_type/:tenant_id/:key", w.handleFileSave)

	api.Post("/files/zip/:agent_type/:tenant_id/:key", w.handleZipFiles)

	api.Post("/files/unzip/:agent_type/:tenant_id/:key", w.handleUnzipFile)

	api.Post("/files/open/:agent_type/:tenant_id/:key", w.handleOpenFile)

	api.Post("/files/paste/:agent_type/:tenant_id/:key", w.handlePasteFiles)
	DebugPrintf("Starting server with debug mode: %v\n", DebugMode)
	log.Println("WebSocket server started on ws://localhost:80/ws")
	log.Fatal(app.ListenTLS("0.0.0.0:443", "certs/cert.pem", "certs/key.pem"))
	return nil
}

func (w websocketRelay) Initialize() {
	w.SendConnectedClients()
	w.initHandlers()

}

func (w websocketRelay) handleGetAgentDetails(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")

	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	message := map[string]interface{}{
		"type": "get_agent_details",
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleEncoderSettings(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")

	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type":     "encoder_settings",
		"settings": requestBody,
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)

	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleGetInstalledSoftware(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")
	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	message := map[string]interface{}{
		"type": "get_installed_software",
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) sendToAgentAndWaitResponse(tenantID, agentType, key string, message map[string]interface{}) (map[string]interface{}, error) {
	fmt.Printf("Looking for connection - TenantID: %s, AgentType: %s, Key: %s\n", tenantID, agentType, key)

	privateKey := gbl.Store.GetPrivateKeyUsingPublicKey(key)
	DebugPrintf("Private key resolved: %s\n", privateKey)

	gbl.ReverseClientsMutex.RLock()
	_, tenantExists := gbl.ReverseClients[tenantID]
	gbl.ReverseClientsMutex.RUnlock()

	if !tenantExists {
		DebugPrintf("Tenant %s not found in ReverseClients\n", tenantID)
		return nil, fmt.Errorf("no tenant found: %s", tenantID)
	}

	var connection chan []byte
	var agentExists bool

	gbl.ReverseClientsMutex.RLock()
	if agentMap, ok := gbl.ReverseClients[tenantID][agentType]; ok {
		connection, agentExists = agentMap[privateKey]
	}

	if !agentExists {
		if agentType == "file_agent" {
			if agentMap, ok := gbl.ReverseClients[tenantID]["c_agent"]; ok {
				if conn, ok := agentMap[privateKey]; ok {
					connection = conn
					agentExists = true
					agentType = "c_agent"
					DebugPrintf("Fell back to agent type 'c_agent' for tenant %s key %s\n", tenantID, privateKey)
				}
			}
		} else if agentType == "c_agent" {
			if agentMap, ok := gbl.ReverseClients[tenantID]["file_agent"]; ok {
				if conn, ok := agentMap[privateKey]; ok {
					connection = conn
					agentExists = true
					agentType = "file_agent"
					DebugPrintf("Fell back to agent type 'file_agent' for tenant %s key %s\n", tenantID, privateKey)
				}
			}
		}
	}
	gbl.ReverseClientsMutex.RUnlock()

	if !agentExists {
		DebugPrintf("Available keys for tenant %s %s: %v\n", tenantID, agentType, getKeysForTenantAndAgent(tenantID, agentType))
		return nil, fmt.Errorf("no %s found for key: %s (private: %s)", agentType, key, privateKey)
	}

	DebugPrintf("Connection found, sending message\n")

	responseChannel := make(chan map[string]interface{}, 1)
	requestID := fmt.Sprintf("%d", time.Now().UnixNano())
	message["request_id"] = requestID
	DebugPrintf("Request ID: %s\n", requestID)

	gbl.PendingHTTPRequestsMutex.Lock()
	gbl.PendingHTTPRequests[requestID] = responseChannel
	gbl.PendingHTTPRequestsMutex.Unlock()

	data, err := json.Marshal(message)
	if err != nil {
		gbl.PendingHTTPRequestsMutex.Lock()
		delete(gbl.PendingHTTPRequests, requestID)
		gbl.PendingHTTPRequestsMutex.Unlock()
		return nil, fmt.Errorf("failed to marshal message: %v", err)
	}

	select {
	case connection <- data:

	case <-time.After(5 * time.Second):
		gbl.PendingHTTPRequestsMutex.Lock()
		delete(gbl.PendingHTTPRequests, requestID)
		gbl.PendingHTTPRequestsMutex.Unlock()
		DebugPrintf("Timeout sending message to agent\n")
		return nil, fmt.Errorf("timeout sending message to agent")
	}

	select {
	case response := <-responseChannel:
		gbl.PendingHTTPRequestsMutex.Lock()
		delete(gbl.PendingHTTPRequests, requestID)
		gbl.PendingHTTPRequestsMutex.Unlock()
		fmt.Printf("Response received.")
		DebugPrintf("Response: %v\n", response)
		return response, nil
	case <-time.After(30 * time.Second):
		gbl.PendingHTTPRequestsMutex.Lock()
		delete(gbl.PendingHTTPRequests, requestID)
		gbl.PendingHTTPRequestsMutex.Unlock()
		DebugPrintf("Timeout waiting for agent response\n")
		return nil, fmt.Errorf("timeout waiting for agent response")
	}
}

func getKeysForTenantAndAgent(tenantID, agentType string) []string {
	var keys []string

	gbl.ReverseClientsMutex.RLock()
	defer gbl.ReverseClientsMutex.RUnlock()

	if clients, exists := gbl.ReverseClients[tenantID]; exists {
		if agentClients, exists := clients[agentType]; exists {
			for key := range agentClients {
				keys = append(keys, key)
			}
		}
	}
	return keys
}

func (w websocketRelay) handleGetAgentKeys(c *fiber.Ctx) error {
	tenantID := c.Params("tenant_id")
	devices, err := gbl.Store.DeviceList()
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": "Failed to load agents"})
	}

	if agent, exists := devices[tenantID]; exists {
		if agentMap, ok := agent.(map[string]interface{}); ok {
			if publickey, keyExists := agentMap["publickey"]; keyExists {
				return c.JSON(fiber.Map{
					"tenant_id": tenantID,
					"key":       publickey,
					"publickey": publickey,
				})
			}
		}
	}

	return c.Status(404).JSON(fiber.Map{"error": "Agent not found"})
}
