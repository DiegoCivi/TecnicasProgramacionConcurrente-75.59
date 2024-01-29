ecom1:
	cargo r --bin ecommerce -- txt_files/online_orders1.txt 127.0.0.1 1 127.0.0.2 2 127.0.0.3 3 6000 6001 

ecom2:
	cargo r --bin ecommerce -- txt_files/online_orders2.txt 127.0.0.2 2 127.0.0.1 1 127.0.0.3 3 6000 6001 

ecom3:
	cargo r --bin ecommerce -- txt_files/online_orders3.txt 127.0.0.3 3 127.0.0.1 1 127.0.0.2 2 6000 6001 
	
store1:
	cargo r --bin store -- txt_files/stock.txt txt_files/client_orders.txt 1 3 6001 127.0.0.1 1 127.0.0.2 2 127.0.0.3 3

store2:
	cargo r --bin store -- txt_files/stock2.txt txt_files/client_orders2.txt 2 3 6001 127.0.0.1 1 127.0.0.2 2 127.0.0.3 3
