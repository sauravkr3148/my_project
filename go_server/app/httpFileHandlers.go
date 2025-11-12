package relay

import (
	"encoding/base64"
	"fmt"
	"path/filepath"

	"github.com/gofiber/fiber/v2"
)

func (w websocketRelay) handleListFiles(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")
	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type":        "list_remote",
		"path":        requestBody["path"],
		"show_hidden": requestBody["show_hidden"],
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleFileUpload(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")
	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	file, err := c.FormFile("file")
	if err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "No file provided"})
	}

	path := c.FormValue("path")
	if path == "" {
		return c.Status(400).JSON(fiber.Map{"error": "No path provided"})
	}

	fileContent, err := file.Open()
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": "Failed to open file"})
	}
	defer fileContent.Close()

	fileBytes := make([]byte, file.Size)
	_, err = fileContent.Read(fileBytes)
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": "Failed to read file"})
	}

	contentBase64 := base64.StdEncoding.EncodeToString(fileBytes)

	message := map[string]interface{}{
		"type":           "upload_file",
		"path":           path,
		"filename":       file.Filename,
		"content_base64": contentBase64,
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleFileDownload(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")
	tenantID := c.Params("tenant_id")
	key := c.Params("key")
	path := c.Query("path")

	message := map[string]interface{}{
		"type": "download_file",
		"path": path,
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	if content, exists := response["content"]; exists {
		if contentStr, ok := content.(string); ok {

			fileBytes, err := base64.StdEncoding.DecodeString(contentStr)
			if err != nil {
				return c.Status(500).JSON(fiber.Map{"error": "Failed to decode file content"})
			}

			filename := filepath.Base(path)

			c.Set("Content-Disposition", fmt.Sprintf("attachment; filename=\"%s\"", filename))
			c.Set("Content-Type", "application/octet-stream")

			return c.Send(fileBytes)
		}
	}

	return c.JSON(response)
}

func (w websocketRelay) handleFileDelete(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")
	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type": "delete",
		"path": requestBody["path"],
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleCreateDirectory(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")
	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type": "create_folder",
		"path": requestBody["path"],
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleFileRename(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")

	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type":     "rename",
		"old_path": requestBody["old_path"],
		"new_name": requestBody["new_name"],
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)

	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleFileEdit(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")

	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type": "edit_file",
		"path": requestBody["path"],
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)

	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleFileSave(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")

	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type":    "save_file",
		"path":    requestBody["path"],
		"content": requestBody["content"],
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)

	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleZipFiles(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")

	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type":        "zip_file",
		"target_list": requestBody["target_list"],
		"zip_name":    requestBody["zip_name"],
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)

	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleUnzipFile(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")

	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type":   "unzip_file",
		"source": requestBody["source"],
		"target": requestBody["target"],
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)

	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleOpenFile(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")

	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type": "open_file",
		"path": requestBody["path"],
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handlePasteFiles(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")

	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	message := map[string]interface{}{
		"type":      "paste_file",
		"from_list": requestBody["from_list"],
		"to":        requestBody["to"],
		"mode":      requestBody["mode"],
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, message)

	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}

func (w websocketRelay) handleFileOperation(c *fiber.Ctx) error {
	agentType := c.Params("agent_type")

	tenantID := c.Params("tenant_id")
	key := c.Params("key")

	var requestBody map[string]interface{}
	if err := c.BodyParser(&requestBody); err != nil {
		return c.Status(400).JSON(fiber.Map{"error": "Invalid request body"})
	}

	response, err := w.sendToAgentAndWaitResponse(tenantID, agentType, key, requestBody)
	if err != nil {
		return c.Status(500).JSON(fiber.Map{"error": err.Error()})
	}

	return c.JSON(response)
}
